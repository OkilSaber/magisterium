//! Client pour toute API compatible OpenAI : OpenAI, serveurs locaux (LM Studio,
//! Ollama…), OpenRouter, Groq, Mistral, DeepSeek, Gemini…

use super::sse::{self, Sse};
use super::{Chunk, ModelInfo, Sink};
use crate::tools::WebTools;
use serde_json::{json, Value};
use std::time::Duration;

fn client(key: Option<&str>) -> Result<reqwest::header::HeaderMap, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(key) = key.filter(|k| !k.is_empty()) {
        let value = format!("Bearer {key}")
            .parse()
            .map_err(|_| "Clé API invalide")?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    Ok(headers)
}

pub async fn list_models(base: &str, key: Option<&str>) -> Result<Vec<ModelInfo>, String> {
    let resp = reqwest::Client::new()
        .get(format!("{base}/models"))
        .headers(client(key)?)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| sse::unreachable(base, &e))?;
    if !resp.status().is_success() {
        return Err(sse::http_error(resp).await);
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    let mut models: Vec<ModelInfo> = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["id"].as_str()?;
            // Les modèles d'embedding ne savent pas discuter.
            if id.contains("embed") {
                return None;
            }
            let label = m["name"].as_str().unwrap_or(id);
            Some(ModelInfo::new(id, label))
        })
        .collect();
    models.sort_by_key(|a| a.label.to_lowercase());
    models.dedup_by(|a, b| a.id == b.id);

    // Serveur LM Studio : on récupère aussi les vrais noms et l'état chargé.
    if let Some(native) = lmstudio::models(base, key).await {
        for m in &mut models {
            if let Some(info) = native.iter().find(|n| n.key == m.id) {
                m.label = info.display_name.clone();
                m.loaded = Some(info.loaded);
            }
        }
    }
    Ok(models)
}

/// Nombre maximal d'allers-retours avec les outils pour une même réponse.
const MAX_TOOL_ROUNDS: usize = 8;

pub async fn stream_chat(
    base: &str,
    key: Option<&str>,
    model: &str,
    prompt: &str,
    tools: Option<&WebTools>,
    on_delta: Sink<'_>,
) -> Result<String, String> {
    let mut messages = Vec::new();
    if let Some(t) = tools {
        messages.push(json!({ "role": "system", "content": t.system_prompt() }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));
    let mut offer_tools = tools.is_some();

    for _ in 0..MAX_TOOL_ROUNDS {
        let defs = tools
            .filter(|_| offer_tools)
            .map(|t| t.openai_definitions());
        let resp = match send_chat(base, key, model, &messages, defs.as_ref()).await? {
            Ok(resp) => resp,
            // Le chargement « à la volée » de LM Studio échoue parfois là où un
            // chargement explicite passe : on le charge nous-mêmes, puis on réessaie.
            Err(err) if err.contains("Failed to load model") => {
                lmstudio::load(base, key, model).await.map_err(|load_err| {
                    format!("{err}. Chargement explicite impossible : {load_err}")
                })?;
                continue;
            }
            // Modèle ou fournisseur sans appel d'outils : on répond sans le web.
            Err(err) if offer_tools && rejects_tools(&err) => {
                offer_tools = false;
                continue;
            }
            Err(err) => return Err(err),
        };

        let mut state = StreamState::default();
        let text = sse::read(resp, &mut |data| state.parse(data), on_delta).await?;
        if state.calls.is_empty() {
            return Ok(text);
        }

        let web = tools.expect("des appels d'outils supposent des outils proposés");
        messages.push(json!({
            "role": "assistant",
            "content": if text.is_empty() { Value::Null } else { Value::String(text) },
            "tool_calls": state.calls.iter().map(|c| json!({
                "id": c.id,
                "type": "function",
                "function": { "name": c.name, "arguments": c.arguments },
            })).collect::<Vec<_>>(),
        }));
        for call in &state.calls {
            let detail = tool_detail(&call.arguments);
            on_delta(Chunk::Tool {
                name: &call.name,
                detail: &detail,
            });
            let (_, result) = web.execute(&call.name, &call.arguments).await;
            messages.push(json!({ "role": "tool", "tool_call_id": call.id, "content": result }));
        }
    }
    Err("Trop d'appels d'outils sans réponse finale".into())
}

/// Requête ou URL d'un appel d'outil, pour l'afficher pendant qu'il s'exécute.
fn tool_detail(arguments: &str) -> String {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    args["query"]
        .as_str()
        .or_else(|| args["url"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn rejects_tools(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("tool")
        && (e.contains("support") || e.contains("not allowed") || e.contains("unsupported"))
}

/// Envoie la requête ; l'erreur HTTP éventuelle est renvoyée à part pour
/// pouvoir réagir à son contenu.
async fn send_chat(
    base: &str,
    key: Option<&str>,
    model: &str,
    messages: &[Value],
    tools: Option<&Value>,
) -> Result<Result<reqwest::Response, String>, String> {
    let mut body = json!({ "model": model, "stream": true, "messages": messages });
    if let Some(tools) = tools {
        body["tools"] = tools.clone();
    }
    let resp = reqwest::Client::new()
        .post(format!("{base}/chat/completions"))
        .headers(client(key)?)
        .json(&body)
        .send()
        .await
        .map_err(|e| sse::unreachable(base, &e))?;
    Ok(if resp.status().is_success() {
        Ok(resp)
    } else {
        Err(sse::http_error(resp).await)
    })
}

#[derive(Default, Debug)]
struct ToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Les appels d'outils arrivent en morceaux, repérés par leur `index`.
#[derive(Default)]
struct StreamState {
    calls: Vec<ToolCall>,
}

impl StreamState {
    fn parse(&mut self, data: &str) -> Sse {
        if let Ok(v) = serde_json::from_str::<Value>(data) {
            for part in v["choices"][0]["delta"]["tool_calls"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let index = part["index"].as_u64().unwrap_or(0) as usize;
                while self.calls.len() <= index {
                    self.calls.push(ToolCall::default());
                }
                let call = &mut self.calls[index];
                if let Some(id) = part["id"].as_str() {
                    call.id = id.to_string();
                }
                let f = &part["function"];
                call.name.push_str(f["name"].as_str().unwrap_or_default());
                call.arguments
                    .push_str(f["arguments"].as_str().unwrap_or_default());
            }
        }
        parse_sse_data(data)
    }
}

/// API native de LM Studio (`/api/v1/…`), à côté de l'API compatible OpenAI.
mod lmstudio {
    use super::super::sse;
    use super::client;
    use serde_json::{json, Value};
    use std::time::Duration;

    /// Contexte de repli quand le chargement avec les réglages par défaut échoue.
    const FALLBACK_CONTEXT: u32 = 16384;

    pub struct NativeModel {
        pub key: String,
        pub display_name: String,
        pub loaded: bool,
    }

    /// `http://hôte:1234/v1` → `http://hôte:1234`.
    fn root(base: &str) -> Option<&str> {
        base.strip_suffix("/v1")
    }

    pub async fn models(base: &str, key: Option<&str>) -> Option<Vec<NativeModel>> {
        let resp = reqwest::Client::new()
            .get(format!("{}/api/v1/models", root(base)?))
            .headers(client(key).ok()?)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: Value = resp.json().await.ok()?;
        Some(
            body["models"]
                .as_array()?
                .iter()
                .filter_map(|m| {
                    Some(NativeModel {
                        key: m["key"].as_str()?.to_string(),
                        display_name: m["display_name"].as_str()?.to_string(),
                        loaded: m["loaded_instances"]
                            .as_array()
                            .is_some_and(|a| !a.is_empty()),
                    })
                })
                .collect(),
        )
    }

    pub async fn load(base: &str, key: Option<&str>, model: &str) -> Result<(), String> {
        let root = root(base).ok_or("serveur non LM Studio")?;
        match load_with(root, key, model, None).await {
            Ok(()) => Ok(()),
            Err(_) => load_with(root, key, model, Some(FALLBACK_CONTEXT)).await,
        }
    }

    async fn load_with(
        root: &str,
        key: Option<&str>,
        model: &str,
        context_length: Option<u32>,
    ) -> Result<(), String> {
        let mut body = json!({ "model": model });
        if let Some(ctx) = context_length {
            body["context_length"] = json!(ctx);
        }
        let resp = reqwest::Client::new()
            .post(format!("{root}/api/v1/models/load"))
            .headers(client(key)?)
            .json(&body)
            .timeout(Duration::from_secs(300))
            .send()
            .await
            .map_err(|e| sse::unreachable(root, &e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(sse::http_error(resp).await)
        }
    }
}

fn parse_sse_data(data: &str) -> Sse {
    if data == "[DONE]" {
        return Sse::Done;
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else {
        return Sse::Ignore;
    };
    if let Some(err) = v.get("error") {
        return Sse::Error(
            err["message"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| err.to_string()),
        );
    }
    let delta = &v["choices"][0]["delta"];
    if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
        return Sse::Delta(text.into());
    }
    // OpenRouter : `reasoning` ; DeepSeek, LM Studio, vLLM : `reasoning_content`.
    match delta["reasoning"]
        .as_str()
        .or_else(|| delta["reasoning_content"].as_str())
    {
        Some(text) if !text.is_empty() => Sse::Thinking(text.into()),
        _ => Sse::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sse_chunks() {
        assert_eq!(
            parse_sse_data(r#"{"choices":[{"delta":{"content":"Salut"}}]}"#),
            Sse::Delta("Salut".into())
        );
        assert_eq!(
            parse_sse_data(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#),
            Sse::Ignore
        );
        assert_eq!(parse_sse_data("[DONE]"), Sse::Done);
        let mut state = StreamState::default();
        state.parse(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"web_search","arguments":"{\"que"}}]}}]}"#);
        state.parse(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ry\":\"x\"}"}}]}}]}"#);
        assert_eq!(state.calls.len(), 1);
        assert_eq!(state.calls[0].id, "c1");
        assert_eq!(state.calls[0].name, "web_search");
        assert_eq!(state.calls[0].arguments, r#"{"query":"x"}"#);
        assert_eq!(
            parse_sse_data(r#"{"choices":[{"delta":{"content":"","reasoning":"Hmm"}}]}"#),
            Sse::Thinking("Hmm".into())
        );
        assert_eq!(
            parse_sse_data(r#"{"choices":[{"delta":{"reasoning_content":"Voyons"}}]}"#),
            Sse::Thinking("Voyons".into())
        );
        assert_eq!(
            parse_sse_data(r#"{"error":{"message":"model not found"}}"#),
            Sse::Error("model not found".into())
        );
    }
}

#[cfg(test)]
mod live {
    /// `LMS_URL=http://hôte:1234/v1 cargo test live_lmstudio -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_lmstudio_models_and_load() {
        let url = std::env::var("LMS_URL").unwrap();
        for m in super::list_models(&url, None).await.unwrap() {
            println!("{} | {} | chargé={:?}", m.id, m.label, m.loaded);
        }
        if let Ok(model) = std::env::var("LMS_LOAD") {
            println!(
                "chargement explicite : {:?}",
                super::lmstudio::load(&url, None, &model).await
            );
        }
    }
}

#[cfg(test)]
mod live_tools {
    use crate::providers::Chunk;
    use crate::tools::WebTools;

    /// LM Studio local + un modèle qui sait appeler des outils : il doit lire la page.
    #[tokio::test]
    #[ignore]
    async fn live_fetch_url_tool() {
        let tools = WebTools {
            engine: None,
            today: "2026-09-25".into(),
        };
        let on = |c: Chunk<'_>| {
            if let Chunk::Tool { name, detail } = c {
                println!("OUTIL {name} {detail}");
            }
        };
        let r = super::stream_chat(
            "http://localhost:1234/v1",
            None,
            "essentialai/rnj-1",
            "Lis la page https://example.com avec l'outil fetch_url et donne-moi son titre exact.",
            Some(&tools),
            &on,
        )
        .await;
        println!("RÉPONSE {r:?}");
    }
}
