pub mod anthropic;
pub mod antigravity;
pub mod claude;
mod cli;
pub mod openai_compat;
mod sse;

use crate::config::{self, ApiKind, ProviderConfig};
use crate::tools::WebTools;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;

pub const CLAUDE_CLI: &str = "claude-cli";
pub const ANTIGRAVITY_CLI: &str = "antigravity-cli";

/// Ce que les agents CLI ont le droit de faire, réglé pour tout le conseil.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecMode {
    /// Lecture seule : les agents analysent et proposent, sans rien modifier.
    Plan,
    /// Éditions de fichiers acceptées dans le dossier de travail.
    Edit,
    /// Claude décide seul de ce qui est sûr ; Antigravity passe en `Edit`.
    Auto,
    /// Aucune demande de permission (`--dangerously-skip-permissions`).
    Full,
}

#[derive(Serialize, Clone, Debug)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    /// Modèle déjà chargé en mémoire (connu pour les serveurs LM Studio).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded: Option<bool>,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            loaded: None,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    /// « cli » (détecté sur la machine) ou préréglage API (« openai », « lmstudio »…).
    pub preset: String,
    pub available: bool,
    /// Raison d'indisponibilité ou avertissement.
    pub status: Option<String>,
    pub models: Vec<ModelInfo>,
    /// L'agent peut lire/écrire des fichiers et lancer des commandes.
    pub uses_tools: bool,
    /// Niveaux de réflexion acceptés (vide : pas de réglage).
    pub efforts: Vec<&'static str>,
}

/// Comment joindre un fournisseur, résolu avant le lancement d'une session.
#[derive(Clone, Debug)]
pub enum Backend {
    ClaudeCli,
    AntigravityCli,
    Api {
        kind: ApiKind,
        base_url: String,
        key: Option<String>,
    },
}

pub struct RunCtx<'a> {
    pub workdir: &'a Path,
    pub mode: ExecMode,
    /// Outils web proposés aux modèles API (les CLI ont les leurs).
    pub web: Option<&'a WebTools>,
}

/// Morceau reçu au fil du streaming : la réponse, ou le raisonnement du modèle
/// quand le fournisseur le transmet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chunk<'a> {
    Text(&'a str),
    Thinking(&'a str),
    /// Le modèle utilise un outil (`web_search`, `fetch_url`) ; `detail` : requête ou URL.
    Tool {
        name: &'a str,
        detail: &'a str,
    },
}

/// Reçoit chaque morceau au fil du streaming.
pub type Sink<'a> = &'a (dyn Fn(Chunk<'_>) + Send + Sync);

/// Les CLI installées puis les fournisseurs configurés. Une CLI absente n'apparaît pas.
pub async fn detect_all(app: &AppHandle) -> Vec<ProviderInfo> {
    let (claude, agy) = tokio::join!(claude::detect(), antigravity::detect());
    let configs = config::load(app).unwrap_or_default();
    let apis = futures_util::future::join_all(configs.iter().map(detect_api)).await;
    claude.into_iter().chain(agy).chain(apis).collect()
}

pub async fn detect_one(app: &AppHandle, id: &str) -> Result<ProviderInfo, String> {
    match id {
        CLAUDE_CLI => claude::detect()
            .await
            .ok_or("Claude Code CLI introuvable".into()),
        ANTIGRAVITY_CLI => antigravity::detect()
            .await
            .ok_or("Antigravity CLI introuvable".into()),
        _ => Ok(detect_api(&config::find(app, id)?).await),
    }
}

async fn detect_api(cfg: &ProviderConfig) -> ProviderInfo {
    let key = config::key_for(cfg);
    let models = list_api_models(cfg.kind, &cfg.base_url, key.as_deref()).await;
    let (available, status, models) = match models {
        Ok(m) if m.is_empty() => (false, Some("Aucun modèle disponible".into()), m),
        Ok(m) => (true, None, m),
        Err(e) => (false, Some(e), Vec::new()),
    };
    ProviderInfo {
        id: cfg.id.clone(),
        name: cfg.name.clone(),
        preset: cfg.preset.clone(),
        available,
        status,
        models,
        uses_tools: false,
        efforts: Vec::new(),
    }
}

pub async fn list_api_models(
    kind: ApiKind,
    base_url: &str,
    key: Option<&str>,
) -> Result<Vec<ModelInfo>, String> {
    let base = config::normalize_url(base_url);
    match kind {
        ApiKind::Openai => openai_compat::list_models(&base, key).await,
        ApiKind::Anthropic => anthropic::list_models(&base, key).await,
    }
}

/// Backend d'une CLI, sans passer par la config (utilisé aussi par les tests).
pub fn cli_backend(id: &str) -> Option<Backend> {
    match id {
        CLAUDE_CLI => Some(Backend::ClaudeCli),
        ANTIGRAVITY_CLI => Some(Backend::AntigravityCli),
        _ => None,
    }
}

pub fn resolve(app: &AppHandle, id: &str) -> Result<Backend, String> {
    if let Some(b) = cli_backend(id) {
        return Ok(b);
    }
    let cfg = config::find(app, id)?;
    let key = config::key_for(&cfg);
    Ok(Backend::Api {
        kind: cfg.kind,
        base_url: cfg.base_url,
        key,
    })
}

/// Lance un prompt et renvoie la réponse finale.
pub async fn run(
    backend: &Backend,
    model: &str,
    effort: &str,
    prompt: &str,
    ctx: &RunCtx<'_>,
    on_delta: Sink<'_>,
) -> Result<String, String> {
    match backend {
        Backend::ClaudeCli => claude::run(model, effort, prompt, ctx, on_delta).await,
        Backend::AntigravityCli => antigravity::run(model, prompt, ctx, on_delta).await,
        Backend::Api {
            kind: ApiKind::Openai,
            base_url,
            key,
        } => {
            openai_compat::stream_chat(base_url, key.as_deref(), model, prompt, ctx.web, on_delta)
                .await
        }
        Backend::Api {
            kind: ApiKind::Anthropic,
            base_url,
            key,
        } => anthropic::stream(base_url, key.as_deref(), model, prompt, on_delta).await,
    }
}
