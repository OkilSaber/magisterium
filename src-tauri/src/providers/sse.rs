//! Lecture d'un flux Server-Sent Events, commun aux API OpenAI et Anthropic.

use super::{Chunk, Sink};
use futures_util::StreamExt;
use std::time::Duration;

/// Interprétation d'une ligne `data:` par le fournisseur.
#[derive(Debug, PartialEq)]
pub enum Sse {
    Delta(String),
    /// Raisonnement du modèle (non inclus dans la réponse finale).
    Thinking(String),
    Done,
    Error(String),
    Ignore,
}

/// Silence maximal entre deux morceaux du flux avant d'abandonner.
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// Lit le flux jusqu'au bout (ou jusqu'à `Done`) et renvoie le texte complet.
pub async fn read(
    resp: reqwest::Response,
    parse: &mut (dyn FnMut(&str) -> Sse + Send),
    on_delta: Sink<'_>,
) -> Result<String, String> {
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut out = String::new();

    loop {
        let chunk = match tokio::time::timeout(IDLE_TIMEOUT, stream.next()).await {
            Ok(Some(chunk)) => chunk.map_err(|e| e.to_string())?,
            Ok(None) => break,
            Err(_) => {
                return Err(format!(
                    "Aucune donnée reçue depuis {} s, requête abandonnée",
                    IDLE_TIMEOUT.as_secs()
                ))
            }
        };
        buf.extend_from_slice(&chunk);
        // Découpe sur les octets : un caractère UTF-8 peut tomber à cheval sur deux morceaux.
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&raw);
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            match parse(data.trim()) {
                Sse::Delta(text) => {
                    out.push_str(&text);
                    on_delta(Chunk::Text(&text));
                }
                Sse::Thinking(text) => on_delta(Chunk::Thinking(&text)),
                Sse::Done => return Ok(out),
                Sse::Error(e) => return Err(e),
                Sse::Ignore => {}
            }
        }
    }
    Ok(out)
}

/// Message d'erreur lisible à partir d'une réponse HTTP en échec.
pub async fn http_error(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or_else(|| v["error"].as_str())
                .or_else(|| v["message"].as_str())
                .map(String::from)
        })
        .unwrap_or(body);
    format!("HTTP {status} : {message}")
}

/// Erreur de connexion avec sa cause racine (« error sending request » seul
/// n'aide personne), et une piste quand macOS bloque le réseau local.
pub fn unreachable(base: &str, err: &reqwest::Error) -> String {
    let mut root: &dyn std::error::Error = err;
    while let Some(next) = root.source() {
        root = next;
    }
    let cause = root.to_string();
    let mut msg = format!("Serveur injoignable ({base}) : {cause}");
    if cause.contains("os error 65") || cause.contains("No route to host") {
        msg.push_str(
            ". macOS bloque sans doute l'accès au réseau local : autorise Magisterium dans \
             Réglages Système › Confidentialité et sécurité › Réseau local.",
        );
    }
    msg
}
