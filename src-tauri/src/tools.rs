//! Outils web donnés aux modèles API et locaux (les CLI ont les leurs) :
//! `web_search` via Tavily ou SearXNG, et `fetch_url` pour lire une page.

use serde_json::{json, Value};
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

/// Outils disponibles pour une session. Sans moteur, seul `fetch_url` est proposé.
#[derive(Clone, Debug)]
pub struct WebTools {
    pub engine: Option<SearchEngine>,
    /// Date du jour, pour que le modèle sache ce que « récent » veut dire.
    pub today: String,
}

pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

impl WebTools {
    /// Définitions au format « function calling » d'OpenAI.
    pub fn openai_definitions(&self) -> Value {
        let mut defs = Vec::new();
        if self.engine.is_some() {
            defs.push(json!({
                "type": "function",
                "function": {
                    "name": "web_search",
                    "description": "Search the web. Returns titles, URLs and snippets of the top results.",
                    "parameters": {
                        "type": "object",
                        "properties": { "query": { "type": "string", "description": "Search query" } },
                        "required": ["query"]
                    }
                }
            }));
        }
        defs.push(json!({
            "type": "function",
            "function": {
                "name": "fetch_url",
                "description": "Download a web page and return its readable text.",
                "parameters": {
                    "type": "object",
                    "properties": { "url": { "type": "string", "description": "Absolute http(s) URL" } },
                    "required": ["url"]
                }
            }
        }));
        Value::Array(defs)
    }

    /// Consigne système : date du jour et quand utiliser les outils.
    pub fn system_prompt(&self) -> String {
        let search = if self.engine.is_some() {
            "`web_search` to find sources and "
        } else {
            ""
        };
        format!(
            "Today is {}. You have internet access through tools: use {search}`fetch_url` to read \
             pages. Use them whenever the question involves recent events, facts you are unsure \
             about, or anything that should be checked; cite the URLs you relied on. Answer in \
             the user's language.",
            self.today
        )
    }

    /// Exécute un appel d'outil. Renvoie ce qui est montré à l'utilisateur et le
    /// résultat transmis au modèle (une erreur devient un résultat lisible).
    pub async fn execute(&self, name: &str, arguments: &str) -> (String, String) {
        let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
        match name {
            "web_search" => {
                let query = args["query"].as_str().unwrap_or_default().to_string();
                let result = match &self.engine {
                    Some(engine) => search(engine, &query).await.map(|hits| format_hits(&hits)),
                    None => Err("web search is not configured".into()),
                };
                (query, result.unwrap_or_else(|e| format!("Error: {e}")))
            }
            "fetch_url" => {
                let url = args["url"].as_str().unwrap_or_default().to_string();
                let result = fetch(&url).await;
                (url, result.unwrap_or_else(|e| format!("Error: {e}")))
            }
            other => (other.to_string(), format!("Error: unknown tool {other}")),
        }
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

    #[test]
    fn offers_search_only_with_an_engine() {
        let tools = WebTools {
            engine: None,
            today: "2026-09-25".into(),
        };
        assert_eq!(tools.openai_definitions().as_array().unwrap().len(), 1);
        let tools = WebTools {
            engine: Some(SearchEngine::Searxng {
                url: "http://x".into(),
            }),
            ..tools
        };
        assert_eq!(tools.openai_definitions().as_array().unwrap().len(), 2);
        assert!(tools.system_prompt().contains("2026-09-25"));
    }
}
