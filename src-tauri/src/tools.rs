//! Outils web donnés aux modèles API et locaux (les CLI ont les leurs) :
//! `web_search` via Tavily ou SearXNG, et `fetch_url` pour lire une page.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

/// Longueur maximale d'une page renvoyée au modèle.
const MAX_PAGE_CHARS: usize = 15_000;
/// Taille maximale téléchargée pour une page.
const MAX_PAGE_BYTES: usize = 3 * 1024 * 1024;
const RESULTS: usize = 6;

#[derive(Clone, Debug)]
pub enum SearchEngine {
    Tavily { key: String },
    Searxng { url: String },
}

/// Outils proposés aux modèles API et locaux pour une session : le web (si activé)
/// et la lecture du dossier de travail (si l'utilisateur en a choisi un).
#[derive(Clone, Debug)]
pub struct ModelTools {
    /// Outils web activés ; sans moteur, seul `fetch_url` est proposé.
    pub web: bool,
    pub engine: Option<SearchEngine>,
    /// Dossier de travail, en lecture seule.
    pub workspace: Option<PathBuf>,
    /// Date du jour, pour que le modèle sache ce que « récent » veut dire.
    pub today: String,
}

pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

fn function(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": { "type": "object", "properties": properties, "required": required }
        }
    })
}

impl ModelTools {
    pub fn is_empty(&self) -> bool {
        !self.web && self.workspace.is_none()
    }

    /// Définitions au format « function calling » d'OpenAI.
    pub fn openai_definitions(&self) -> Value {
        let mut defs = Vec::new();
        if self.web && self.engine.is_some() {
            defs.push(function(
                "web_search",
                "Search the web. Returns titles, URLs and snippets of the top results.",
                json!({ "query": { "type": "string", "description": "Search query" } }),
                &["query"],
            ));
        }
        if self.web {
            defs.push(function(
                "fetch_url",
                "Download a web page and return its readable text.",
                json!({ "url": { "type": "string", "description": "Absolute http(s) URL" } }),
                &["url"],
            ));
        }
        if self.workspace.is_some() {
            defs.push(function(
                "list_dir",
                "List the files and folders of a directory in the user's project.",
                json!({ "path": { "type": "string", "description": "Path relative to the project root, \".\" for the root" } }),
                &[],
            ));
            defs.push(function(
                "read_file",
                "Read a text file of the user's project, optionally a range of lines.",
                json!({
                    "path": { "type": "string", "description": "Path relative to the project root" },
                    "start_line": { "type": "integer", "description": "First line to read (1-based)" },
                    "max_lines": { "type": "integer", "description": "How many lines to read (default 400)" }
                }),
                &["path"],
            ));
            defs.push(function(
                "search_files",
                "Find the lines containing a text in the user's project (case-insensitive).",
                json!({
                    "query": { "type": "string", "description": "Text to look for" },
                    "path": { "type": "string", "description": "Folder to search in, relative to the project root" }
                }),
                &["query"],
            ));
        }
        Value::Array(defs)
    }

    /// Consigne système : date du jour et quand utiliser les outils.
    pub fn system_prompt(&self) -> String {
        let mut s = format!("Today is {}.", self.today);
        if self.web {
            let search = if self.engine.is_some() {
                "`web_search` to find sources and "
            } else {
                ""
            };
            s.push_str(&format!(
                " You have internet access through tools: use {search}`fetch_url` to read pages. \
                 Use them whenever the question involves recent events, facts you are unsure \
                 about, or anything that should be checked; cite the URLs you relied on."
            ));
        }
        if self.workspace.is_some() {
            s.push_str(
                " The user is working in a project folder that you can read (not modify) with \
                 `list_dir`, `read_file` and `search_files`, using paths relative to the project \
                 root. When the request is about \"this repo\", \"this project\" or its code, \
                 explore it with these tools before answering instead of asking the user for it.",
            );
        }
        s.push_str(" Answer in the user's language.");
        s
    }

    /// Exécute un appel d'outil. Renvoie ce qui est montré à l'utilisateur et le
    /// résultat transmis au modèle (une erreur devient un résultat lisible).
    pub async fn execute(&self, name: &str, arguments: &str) -> (String, String) {
        let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
        let arg = |k: &str| args[k].as_str().unwrap_or_default().to_string();
        let (detail, result) = match name {
            "web_search" if self.web => {
                let query = arg("query");
                let result = match &self.engine {
                    Some(engine) => search(engine, &query).await.map(|hits| format_hits(&hits)),
                    None => Err("web search is not configured".into()),
                };
                (query, result)
            }
            "fetch_url" if self.web => {
                let url = arg("url");
                let result = fetch(&url).await;
                (url, result)
            }
            "list_dir" | "read_file" | "search_files" => {
                let path = args["path"].as_str().unwrap_or(".").to_string();
                let result = match &self.workspace {
                    Some(root) => {
                        let (root, name, args) = (root.clone(), name.to_string(), args.clone());
                        tokio::task::spawn_blocking(move || workspace::run(&root, &name, &args))
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                    }
                    None => Err("no project folder is open".into()),
                };
                (
                    if name == "search_files" {
                        arg("query")
                    } else {
                        path
                    },
                    result,
                )
            }
            other => (other.to_string(), Err(format!("unknown tool {other}"))),
        };
        (detail, result.unwrap_or_else(|e| format!("Error: {e}")))
    }
}

/// Lecture du dossier de travail, confinée à sa racine.
mod workspace {
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    const SKIPPED: [&str; 8] = [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        "_build",
        "deps",
        ".venv",
    ];
    const MAX_ENTRIES: usize = 300;
    const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
    const DEFAULT_LINES: usize = 400;
    const MAX_CHARS: usize = 60_000;
    const MAX_HITS: usize = 60;
    const MAX_FILES_SCANNED: usize = 5_000;

    pub fn run(root: &Path, name: &str, args: &Value) -> Result<String, String> {
        let rel = args["path"].as_str().unwrap_or(".");
        let path = resolve(root, rel)?;
        match name {
            "list_dir" => list_dir(root, &path),
            "read_file" => {
                let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
                let lines = args["max_lines"]
                    .as_u64()
                    .map_or(DEFAULT_LINES, |n| n as usize);
                read_file(&path, start, lines)
            }
            _ => search(root, &path, args["query"].as_str().unwrap_or_default()),
        }
    }

    /// Chemin demandé, garanti à l'intérieur de la racine (liens symboliques résolus).
    pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let candidate = root.join(rel.trim_start_matches('/'));
        let path = candidate
            .canonicalize()
            .map_err(|_| format!("{rel}: no such file or directory"))?;
        if path.starts_with(&root) {
            Ok(path)
        } else {
            Err(format!("{rel}: outside of the project folder"))
        }
    }

    fn relative(root: &Path, path: &Path) -> String {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        path.strip_prefix(&root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned())
    }

    fn list_dir(root: &Path, dir: &Path) -> Result<String, String> {
        let mut entries: Vec<(bool, String, u64)> = std::fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let meta = e.metadata().ok()?;
                Some((meta.is_dir(), name, meta.len()))
            })
            .collect();
        entries.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });
        let total = entries.len();
        let mut out = format!("{}/\n", relative(root, dir));
        for (is_dir, name, size) in entries.into_iter().take(MAX_ENTRIES) {
            if is_dir {
                let note = if SKIPPED.contains(&name.as_str()) {
                    "  (skipped in searches)"
                } else {
                    ""
                };
                out.push_str(&format!("  {name}/{note}\n"));
            } else {
                out.push_str(&format!("  {name}  ({size} bytes)\n"));
            }
        }
        if total > MAX_ENTRIES {
            out.push_str(&format!("  … {} more entries\n", total - MAX_ENTRIES));
        }
        Ok(out)
    }

    fn read_text(path: &Path) -> Result<String, String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            return Err("this is a directory, use list_dir".into());
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(format!("file too large ({} bytes)", meta.len()));
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.iter().take(8192).any(|b| *b == 0) {
            return Err("binary file".into());
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn read_file(path: &Path, start: usize, max_lines: usize) -> Result<String, String> {
        let text = read_text(path)?;
        let total = text.lines().count();
        let mut out = String::new();
        for (i, line) in text.lines().enumerate().skip(start - 1).take(max_lines) {
            out.push_str(&format!("{:>5}  {line}\n", i + 1));
            if out.len() > MAX_CHARS {
                out.push_str("[… truncated]\n");
                break;
            }
        }
        let end = (start - 1 + max_lines).min(total);
        if end < total {
            out.push_str(&format!(
                "[lines {start}-{end} of {total}; use start_line to read more]\n"
            ));
        }
        Ok(out)
    }

    fn search(root: &Path, dir: &Path, query: &str) -> Result<String, String> {
        if query.trim().is_empty() {
            return Err("empty query".into());
        }
        let needle = query.to_lowercase();
        let mut hits = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        let mut scanned = 0;
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().into_owned();
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    if !SKIPPED.contains(&name.as_str()) && !name.starts_with('.') {
                        stack.push(path);
                    }
                    continue;
                }
                scanned += 1;
                if scanned > MAX_FILES_SCANNED {
                    break;
                }
                let Ok(text) = read_text(&path) else { continue };
                for (i, line) in text.lines().enumerate() {
                    if line.to_lowercase().contains(&needle) {
                        let line = line.trim();
                        let line: String = line.chars().take(200).collect();
                        hits.push(format!("{}:{}: {line}", relative(root, &path), i + 1));
                        if hits.len() >= MAX_HITS {
                            hits.push(format!("[stopped after {MAX_HITS} matches]"));
                            return Ok(hits.join("\n"));
                        }
                    }
                }
            }
        }
        Ok(if hits.is_empty() {
            "No matches.".into()
        } else {
            hits.join("\n")
        })
    }
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Magisterium/0.1")
        .timeout(Duration::from_secs(20))
        .build()
        .expect("client HTTP")
}

pub async fn search(engine: &SearchEngine, query: &str) -> Result<Vec<Hit>, String> {
    let (resp, results_key, snippet_key) = match engine {
        SearchEngine::Tavily { key } => (
            http()
                .post("https://api.tavily.com/search")
                .bearer_auth(key)
                .json(&json!({ "query": query, "max_results": RESULTS, "search_depth": "basic" }))
                .send()
                .await,
            "results",
            "content",
        ),
        SearchEngine::Searxng { url } => (
            http()
                .get(
                    reqwest::Url::parse_with_params(
                        &format!("{}/search", url.trim_end_matches('/')),
                        &[("q", query), ("format", "json")],
                    )
                    .map_err(|e| e.to_string())?,
                )
                .send()
                .await,
            "results",
            "content",
        ),
    };
    let resp = resp.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let hint = if matches!(engine, SearchEngine::Searxng { .. }) && status.as_u16() == 403 {
            " (active le format JSON dans les réglages de SearXNG)"
        } else {
            ""
        };
        return Err(format!("HTTP {status}{hint}"));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body[results_key]
        .as_array()
        .into_iter()
        .flatten()
        .take(RESULTS)
        .filter_map(|r| {
            Some(Hit {
                title: r["title"].as_str().unwrap_or_default().to_string(),
                url: r["url"].as_str()?.to_string(),
                snippet: r[snippet_key].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect())
}

fn format_hits(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "No results.".into();
    }
    hits.iter()
        .enumerate()
        .map(|(i, h)| format!("{}. {}\n{}\n{}", i + 1, h.title, h.url, h.snippet))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub async fn fetch(url: &str) -> Result<String, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("only http(s) URLs are supported".into());
    }
    let resp = http().get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let is_html = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|ct| ct.contains("html"));
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_PAGE_BYTES)]).into_owned();
    let text = if is_html { html_to_text(&raw) } else { raw };
    Ok(truncate(&text, MAX_PAGE_CHARS))
}

/// Texte lisible d'une page : sans scripts, styles ni navigation, espaces resserrés.
fn html_to_text(html: &str) -> String {
    use scraper::{Html, Node};
    let doc = Html::parse_document(html);
    let mut out = String::new();
    for node in doc.tree.nodes() {
        let Node::Text(text) = node.value() else {
            continue;
        };
        let skipped = node.ancestors().any(|a| {
            a.value().as_element().is_some_and(|e| {
                matches!(
                    e.name(),
                    "script" | "style" | "noscript" | "nav" | "footer" | "svg" | "head"
                )
            })
        });
        if skipped {
            continue;
        }
        let t = text.trim();
        if !t.is_empty() {
            out.push_str(t);
            out.push('\n');
        }
    }
    out
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((i, _)) => format!("{}\n[… page tronquée]", &text[..i]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_readable_text() {
        let html = "<html><head><title>T</title><style>.a{}</style></head><body>\
                    <nav>Menu</nav><h1>Louis de Funès</h1><script>var x=1</script>\
                    <p>Acteur français.</p></body></html>";
        assert_eq!(html_to_text(html), "Louis de Funès\nActeur français.\n");
    }

    #[test]
    fn truncates_on_char_boundary() {
        assert_eq!(truncate("éé", 5), "éé");
        assert!(truncate("ééé", 2).starts_with("éé\n[…"));
    }

    fn names(tools: &ModelTools) -> Vec<String> {
        tools
            .openai_definitions()
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["function"]["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn offers_tools_according_to_settings() {
        let tools = ModelTools {
            web: true,
            engine: None,
            workspace: None,
            today: "2026-09-25".into(),
        };
        assert_eq!(names(&tools), ["fetch_url"]);
        let tools = ModelTools {
            engine: Some(SearchEngine::Searxng {
                url: "http://x".into(),
            }),
            ..tools
        };
        assert_eq!(names(&tools), ["web_search", "fetch_url"]);
        assert!(tools.system_prompt().contains("2026-09-25"));
        let tools = ModelTools {
            web: false,
            workspace: Some(std::env::temp_dir()),
            ..tools
        };
        assert_eq!(names(&tools), ["list_dir", "read_file", "search_files"]);
        assert!(tools.system_prompt().contains("read_file"));
    }

    #[test]
    fn workspace_stays_inside_its_root() {
        let root = std::env::temp_dir().join("magisterium-ws-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    println!(\"Bonjour\");\n}\n",
        )
        .unwrap();
        std::fs::write(root.join("secret.bin"), [0u8, 1, 2]).unwrap();

        let run = |name: &str, args: Value| workspace::run(&root, name, &args);
        assert!(run("list_dir", json!({})).unwrap().contains("src/"));
        assert!(run("read_file", json!({ "path": "src/main.rs" }))
            .unwrap()
            .contains("    2      println!"));
        assert!(run("search_files", json!({ "query": "bonjour" }))
            .unwrap()
            .contains("src/main.rs:2:"));
        assert!(run("read_file", json!({ "path": "secret.bin" })).is_err());
        // Impossible de sortir du dossier, même avec « .. » ou un chemin absolu.
        let escape = format!("{}etc/passwd", "../".repeat(20));
        assert!(run("read_file", json!({ "path": escape }))
            .unwrap_err()
            .contains("outside"));
        assert!(run("list_dir", json!({ "path": ".." }))
            .unwrap_err()
            .contains("outside"));
        assert!(run("list_dir", json!({ "path": "/" })).is_ok_and(|s| s.starts_with("/")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
