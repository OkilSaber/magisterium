//! API Messages d'Anthropic (streaming SSE).

use super::sse::{self, Sse};
use super::{ModelInfo, Sink};
use serde_json::{json, Value};
use std::time::Duration;

const VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 16000;

fn headers(key: Option<&str>) -> Result<reqwest::header::HeaderMap, String> {
    let mut h = reqwest::header::HeaderMap::new();
    let key = key
        .filter(|k| !k.is_empty())
        .ok_or("Clé API Anthropic manquante")?;
    h.insert("x-api-key", key.parse().map_err(|_| "Clé API invalide")?);
    h.insert("anthropic-version", VERSION.parse().unwrap());
    Ok(h)
}

pub async fn list_models(base: &str, key: Option<&str>) -> Result<Vec<ModelInfo>, String> {
    let resp = reqwest::Client::new()
        .get(format!("{base}/models?limit=100"))
        .headers(headers(key)?)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| sse::unreachable(base, &e))?;
    if !resp.status().is_success() {
        return Err(sse::http_error(resp).await);
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["id"].as_str()?;
            Some(ModelInfo::new(id, m["display_name"].as_str().unwrap_or(id)))
        })
        .collect())
}

pub async fn stream(
    base: &str,
    key: Option<&str>,
    model: &str,
    prompt: &str,
    on_delta: Sink<'_>,
) -> Result<String, String> {
    let resp = reqwest::Client::new()
        .post(format!("{base}/messages"))
        .headers(headers(key)?)
        .json(&json!({
            "model": model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "messages": [{ "role": "user", "content": prompt }],
        }))
        .send()
        .await
        .map_err(|e| sse::unreachable(base, &e))?;
    if !resp.status().is_success() {
        return Err(sse::http_error(resp).await);
    }
    sse::read(resp, &mut parse_sse_data, on_delta).await
}

fn parse_sse_data(data: &str) -> Sse {
    let Ok(v) = serde_json::from_str::<Value>(data) else {
        return Sse::Ignore;
    };
    match v["type"].as_str() {
        Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
            Sse::Delta(v["delta"]["text"].as_str().unwrap_or_default().into())
        }
        Some("content_block_delta") if v["delta"]["type"] == "thinking_delta" => {
            Sse::Thinking(v["delta"]["thinking"].as_str().unwrap_or_default().into())
        }
        Some("message_stop") => Sse::Done,
        Some("error") => Sse::Error(
            v["error"]["message"]
                .as_str()
                .unwrap_or("erreur Anthropic")
                .into(),
        ),
        _ => Sse::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_anthropic_events() {
        assert_eq!(
            parse_sse_data(
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Salut"}}"#
            ),
            Sse::Delta("Salut".into())
        );
        assert_eq!(
            parse_sse_data(
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}"#
            ),
            Sse::Thinking("hmm".into())
        );
        assert_eq!(parse_sse_data(r#"{"type":"message_stop"}"#), Sse::Done);
        assert_eq!(
            parse_sse_data(
                r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#
            ),
            Sse::Error("Overloaded".into())
        );
    }
}
