//! Limites d'usage, quand le fournisseur les expose : fenêtres de 5 h et de la
//! semaine des CLI (`/usage`, sans consommer de quota), soldes et plafonds des API.

use crate::config::{self, ProviderConfig};
use crate::providers::{ANTIGRAVITY_CLI, CLAUDE_CLI};
use serde::Serialize;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// Une jauge. `window` est une clé traduite par l'interface.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Meter {
    /// Sous-groupe éventuel (« Gemini Models », « all models »…).
    pub group: Option<String>,
    /// « session », « week », « five_hour », « weekly », « balance », « key_limit »,
    /// « free_daily », « credits ».
    pub window: String,
    /// Part consommée, de 0 à 1.
    pub used: Option<f64>,
    pub amount: Option<f64>,
    pub limit: Option<f64>,
    /// « USD », « CNY », « credits », « requests »…
    pub unit: Option<String>,
    /// Réinitialisation, en ISO 8601.
    pub resets_at: Option<String>,
    /// Réinitialisation en texte libre (quand la CLI ne donne que ça).
    pub resets_text: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProviderUsage {
    pub id: String,
    pub name: String,
    pub preset: String,
    /// « ok », « unlimited », « unavailable » ou « error ».
    pub status: &'static str,
    pub meters: Vec<Meter>,
    pub message: Option<String>,
    /// Tableau de bord du fournisseur, quand il n'y a pas d'API de consommation.
    pub dashboard: Option<&'static str>,
}

impl ProviderUsage {
    fn new(id: &str, name: &str, preset: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            preset: preset.into(),
            status: "ok",
            meters: Vec::new(),
            message: None,
            dashboard: None,
        }
    }

    fn with_result(mut self, result: Result<Vec<Meter>, String>) -> Self {
        match result {
            Ok(meters) if meters.is_empty() => self.status = "unavailable",
            Ok(meters) => self.meters = meters,
            Err(e) => {
                self.status = "error";
                self.message = Some(e);
            }
        }
        self
    }
}

const TIMEOUT: Duration = Duration::from_secs(25);

// ─── CLI ─────────────────────────────────────────────────────────────────

async fn run_cli(mut cmd: Command) -> Result<String, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(TIMEOUT, cmd.output())
        .await
        .map_err(|_| "délai dépassé".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn claude() -> Result<Vec<Meter>, String> {
    let mut cmd = Command::new("claude");
    cmd.args(["-p", "--output-format", "json"])
        .env_remove("CLAUDECODE")
        .current_dir(std::env::temp_dir())
        // Commande locale : ne consomme aucun token.
        .arg("/usage");
    let raw = run_cli(cmd).await?;
    let v: Value = serde_json::from_str(raw.trim()).map_err(|_| "réponse illisible".to_string())?;
    Ok(parse_claude(v["result"].as_str().unwrap_or_default()))
}

/// « Current session: 40% used · resets Sep 25 at 10:29pm (Europe/Paris) »
/// « Current week (all models): 43% used · resets Sep 30 at 7:59pm (Europe/Paris) »
fn parse_claude(text: &str) -> Vec<Meter> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Current ")?;
            let (head, tail) = rest.split_once(':')?;
            let (window, group) = match head.split_once(" (") {
                Some((w, g)) => (w.trim(), Some(g.trim_end_matches(')').to_string())),
                None => (head.trim(), None),
            };
            let (pct, resets) = match tail.split_once('·') {
                Some((p, r)) => (p, r.trim().strip_prefix("resets ").map(String::from)),
                None => (tail, None),
            };
            let pct: f64 = pct.trim().strip_suffix("% used")?.trim().parse().ok()?;
            Some(Meter {
                group,
                window: window.to_string(),
                used: Some(pct / 100.0),
                resets_text: resets,
                ..Meter::default()
            })
        })
        .collect()
}

pub async fn antigravity() -> Result<Vec<Meter>, String> {
    let mut cmd = Command::new("agy");
    cmd.args(["--output-format", "stream-json", "-p=/usage"])
        .current_dir(std::env::temp_dir());
    let raw = run_cli(cmd).await?;
    raw.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["event"] == "command_result")
        .map(|v| parse_antigravity(&v["command"]["data"]))
        .ok_or_else(|| "réponse illisible".to_string())
}

fn parse_antigravity(data: &Value) -> Vec<Meter> {
    let mut meters = Vec::new();
    for group in data["groups"].as_array().into_iter().flatten() {
        for bucket in group["buckets"].as_array().into_iter().flatten() {
            let remaining = bucket["remaining_fraction"].as_f64();
            meters.push(Meter {
                group: group["name"].as_str().map(String::from),
                window: bucket["window"].as_str().unwrap_or("window").to_string(),
                used: remaining.map(|r| (1.0 - r).clamp(0.0, 1.0)),
                resets_at: bucket["reset_time"].as_str().map(String::from),
                ..Meter::default()
            });
        }
    }
    meters
}

// ─── API ─────────────────────────────────────────────────────────────────

async fn get_json(url: &str, key: &str) -> Result<Value, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .bearer_auth(key)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

async fn openrouter(base: &str, key: &str) -> Result<Vec<Meter>, String> {
    let (key_url, credits_url) = (format!("{base}/key"), format!("{base}/credits"));
    let (key_info, credits) = tokio::join!(get_json(&key_url, key), get_json(&credits_url, key));
    let mut meters = Vec::new();
    if let Ok(c) = credits {
        let total = c["data"]["total_credits"].as_f64();
        let used = c["data"]["total_usage"].as_f64();
        if let (Some(total), Some(used)) = (total, used) {
            meters.push(Meter {
                window: "balance".into(),
                used: (total > 0.0).then(|| (used / total).clamp(0.0, 1.0)),
                amount: Some(total - used),
                limit: Some(total),
                unit: Some("USD".into()),
                ..Meter::default()
            });
        }
    }
    let info = key_info?;
    let d = &info["data"];
    if let (Some(limit), Some(usage)) = (d["limit"].as_f64(), d["usage"].as_f64()) {
        meters.push(Meter {
            window: "key_limit".into(),
            used: (limit > 0.0).then(|| (usage / limit).clamp(0.0, 1.0)),
            amount: Some(usage),
            limit: Some(limit),
            unit: Some("USD".into()),
            ..Meter::default()
        });
    }
    let free = &d["free_model_daily_requests"];
    if let (Some(used), Some(limit)) = (free["used"].as_f64(), free["limit"].as_f64()) {
        meters.push(Meter {
            window: "free_daily".into(),
            used: (limit > 0.0).then(|| (used / limit).clamp(0.0, 1.0)),
            amount: Some(used),
            limit: Some(limit),
            unit: Some("requests".into()),
            ..Meter::default()
        });
    }
    Ok(meters)
}

async fn deepseek(base: &str, key: &str) -> Result<Vec<Meter>, String> {
    let root = base.trim_end_matches("/v1");
    let v = get_json(&format!("{root}/user/balance"), key).await?;
    Ok(v["balance_infos"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| Meter {
            window: "balance".into(),
            amount: b["total_balance"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| b["total_balance"].as_f64()),
            unit: b["currency"].as_str().map(String::from),
            ..Meter::default()
        })
        .collect())
}

pub async fn tavily(key: &str) -> Result<Vec<Meter>, String> {
    let v = get_json("https://api.tavily.com/usage", key).await?;
    let (used, limit) = match (
        v["account"]["plan_usage"].as_f64(),
        v["account"]["plan_limit"].as_f64(),
    ) {
        (Some(u), Some(l)) => (u, Some(l)),
        _ => (
            v["key"]["usage"].as_f64().unwrap_or(0.0),
            v["key"]["limit"].as_f64(),
        ),
    };
    Ok(vec![Meter {
        window: "credits".into(),
        used: limit
            .filter(|l| *l > 0.0)
            .map(|l| (used / l).clamp(0.0, 1.0)),
        amount: Some(used),
        limit,
        unit: Some("credits".into()),
        ..Meter::default()
    }])
}

fn is_local(url: &str) -> bool {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    host == "localhost"
        || host.starts_with("127.")
        || host.starts_with("192.168.")
        || host.starts_with("10.")
        || host.ends_with(".local")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|o| o.parse::<u8>().ok())
                .is_some_and(|o| (16..=31).contains(&o)))
}

fn dashboard(preset: &str) -> Option<&'static str> {
    Some(match preset {
        "anthropic" => "https://console.anthropic.com/settings/usage",
        "openai" => "https://platform.openai.com/usage",
        "gemini" => "https://aistudio.google.com/usage",
        "mistral" => "https://console.mistral.ai/usage",
        "groq" => "https://console.groq.com/dashboard/usage",
        "xai" => "https://console.x.ai",
        "together" => "https://api.together.ai/settings/billing",
        "ollama-cloud" => "https://ollama.com/settings",
        "openrouter" => "https://openrouter.ai/activity",
        "deepseek" => "https://platform.deepseek.com/usage",
        _ => return None,
    })
}

async fn api(cfg: &ProviderConfig) -> ProviderUsage {
    let mut usage = ProviderUsage::new(&cfg.id, &cfg.name, &cfg.preset);
    usage.dashboard = dashboard(&cfg.preset);
    if is_local(&cfg.base_url) {
        usage.status = "unlimited";
        return usage;
    }
    let key = config::key_for(cfg);
    let result = match (cfg.preset.as_str(), key.as_deref()) {
        (_, None) => {
            return ProviderUsage {
                status: "unavailable",
                ..usage
            }
        }
        ("openrouter", Some(k)) => openrouter(&cfg.base_url, k).await,
        ("deepseek", Some(k)) => deepseek(&cfg.base_url, k).await,
        _ if cfg.base_url.contains("openrouter.ai") => {
            openrouter(&cfg.base_url, key.as_deref().unwrap()).await
        }
        _ => {
            return ProviderUsage {
                status: "unavailable",
                ..usage
            }
        }
    };
    usage.with_result(result)
}

/// Toutes les limites connues, en parallèle. Les CLI absentes sont ignorées.
pub async fn collect(
    configs: &[ProviderConfig],
    tavily_key: Option<String>,
    searxng: bool,
) -> Vec<ProviderUsage> {
    let claude_cli = async {
        which::which("claude").ok()?;
        Some(ProviderUsage::new(CLAUDE_CLI, "Claude Code CLI", "cli").with_result(claude().await))
    };
    let agy_cli = async {
        which::which("agy").ok()?;
        Some(
            ProviderUsage::new(ANTIGRAVITY_CLI, "Antigravity CLI", "cli")
                .with_result(antigravity().await),
        )
    };
    let tavily_usage = async {
        let key = tavily_key?;
        Some(ProviderUsage::new("web:tavily", "Tavily", "tavily").with_result(tavily(&key).await))
    };
    let (c, a, t, apis) = tokio::join!(
        claude_cli,
        agy_cli,
        tavily_usage,
        futures_util::future::join_all(configs.iter().map(api))
    );
    let mut out: Vec<ProviderUsage> = c.into_iter().chain(a).chain(apis).chain(t).collect();
    if searxng {
        let mut s = ProviderUsage::new("web:searxng", "SearXNG", "searxng");
        s.status = "unlimited";
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_usage_text() {
        let text = "You are currently using your subscription to power your Claude Code usage\n\n\
                    Current session: 40% used · resets Sep 25 at 10:29pm (Europe/Paris)\n\
                    Current week (all models): 43% used · resets Sep 30 at 7:59pm (Europe/Paris)\n\n\
                    Last 24h · 1072 requests · 49 sessions\n  62% of your usage was at >150k context\n";
        let meters = parse_claude(text);
        assert_eq!(meters.len(), 2);
        assert_eq!(meters[0].window, "session");
        assert_eq!(meters[0].used, Some(0.40));
        assert_eq!(
            meters[0].resets_text.as_deref(),
            Some("Sep 25 at 10:29pm (Europe/Paris)")
        );
        assert_eq!(meters[1].window, "week");
        assert_eq!(meters[1].group.as_deref(), Some("all models"));
        assert_eq!(meters[1].used, Some(0.43));
    }

    #[test]
    fn parses_antigravity_usage() {
        let data = serde_json::json!({ "groups": [{ "name": "Gemini Models", "buckets": [
            { "id": "gemini-weekly", "window": "weekly", "remaining_fraction": 0.84, "reset_time": "2026-09-30T18:33:14Z" }
        ]}]});
        let meters = parse_antigravity(&data);
        assert_eq!(meters.len(), 1);
        assert_eq!(meters[0].group.as_deref(), Some("Gemini Models"));
        assert_eq!(meters[0].window, "weekly");
        assert!((meters[0].used.unwrap() - 0.16).abs() < 1e-9);
        assert_eq!(meters[0].resets_at.as_deref(), Some("2026-09-30T18:33:14Z"));
    }

    #[test]
    fn recognizes_local_servers() {
        assert!(is_local("http://localhost:1234/v1"));
        assert!(is_local("http://192.168.31.178:1234/v1"));
        assert!(is_local("http://172.20.0.2:8080/v1"));
        assert!(!is_local("https://openrouter.ai/api/v1"));
        assert!(!is_local("http://172.64.1.1/v1"));
    }

    /// `cargo test live_cli_usage -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_cli_usage() {
        crate::env::init_path();
        println!("claude : {:?}", claude().await);
        println!("agy : {:?}", antigravity().await);
    }
}
