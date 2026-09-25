//! Fournisseurs API configurés par l'utilisateur. Nom, URL et type vont dans
//! `providers.json` ; les clés API vont dans le Trousseau macOS, jamais sur le disque.

use crate::tools::SearchEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

const KEYCHAIN_SERVICE: &str = "dev.magisterium.app";

/// Protocole parlé par le fournisseur.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKind {
    /// `/models` + `/chat/completions` (OpenAI, serveurs locaux, OpenRouter, Groq…).
    Openai,
    /// API Messages d'Anthropic.
    Anthropic,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub kind: ApiKind,
    /// Préréglage d'origine (« openai », « lmstudio », « custom »…), pour l'icône.
    pub preset: String,
    pub base_url: String,
    #[serde(default)]
    pub has_key: bool,
}

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("providers.json"))
}

pub fn load(app: &AppHandle) -> Result<Vec<ProviderConfig>, String> {
    let path = path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_all(app: &AppHandle, configs: &[ProviderConfig]) -> Result<(), String> {
    let path = path(app)?;
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(configs).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn find(app: &AppHandle, id: &str) -> Result<ProviderConfig, String> {
    load(app)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Fournisseur introuvable : {id}"))
}

/// Crée ou met à jour un fournisseur. `api_key` : `None` garde la clé actuelle,
/// `Some("")` la supprime, `Some(clé)` la remplace.
pub fn upsert(
    app: &AppHandle,
    mut config: ProviderConfig,
    api_key: Option<String>,
) -> Result<ProviderConfig, String> {
    config.base_url = normalize_url(&config.base_url);
    if config.id.is_empty() {
        config.id = uuid::Uuid::new_v4().to_string();
    }
    let mut configs = load(app)?;
    let previous = configs.iter().find(|c| c.id == config.id);
    config.has_key = previous.is_some_and(|c| c.has_key);

    match api_key.as_deref().map(str::trim) {
        Some("") => {
            delete_key(&config.id)?;
            config.has_key = false;
        }
        Some(key) => {
            set_key(&config.id, key)?;
            config.has_key = true;
        }
        None => {}
    }

    match configs.iter_mut().find(|c| c.id == config.id) {
        Some(existing) => *existing = config.clone(),
        None => configs.push(config.clone()),
    }
    save_all(app, &configs)?;
    Ok(config)
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut configs = load(app)?;
    configs.retain(|c| c.id != id);
    save_all(app, &configs)?;
    delete_key(id)
}

/// Moteur de recherche des outils web. La clé Tavily va dans le coffre du Trousseau.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WebConfig {
    /// « none », « tavily » ou « searxng ».
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub searxng_url: String,
    #[serde(default)]
    pub has_tavily_key: bool,
    /// SearXNG intégré activé (démarre avec l'app s'il est installé).
    #[serde(default)]
    pub builtin_enabled: bool,
}

const TAVILY_KEY_ID: &str = "web:tavily";

fn web_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(path(app)?.with_file_name("web.json"))
}

pub fn load_web(app: &AppHandle) -> WebConfig {
    let mut web: WebConfig = web_path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    if web.engine.is_empty() {
        web.engine = "none".into();
    }
    web
}

/// `tavily_key` : même convention que pour les fournisseurs (absent = garder).
pub fn save_web(
    app: &AppHandle,
    mut web: WebConfig,
    tavily_key: Option<String>,
) -> Result<WebConfig, String> {
    web.searxng_url = normalize_url(&web.searxng_url);
    let current = load_web(app);
    web.has_tavily_key = current.has_tavily_key;
    // Activé/désactivé ne se change que par l'interrupteur de SearXNG intégré.
    web.builtin_enabled = current.builtin_enabled;
    match tavily_key.as_deref().map(str::trim) {
        Some("") => {
            delete_key(TAVILY_KEY_ID)?;
            web.has_tavily_key = false;
        }
        Some(key) => {
            set_key(TAVILY_KEY_ID, key)?;
            web.has_tavily_key = true;
        }
        None => {}
    }
    write_web(app, &web)?;
    Ok(web)
}

pub fn write_web(app: &AppHandle, web: &WebConfig) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(web).map_err(|e| e.to_string())?;
    std::fs::write(web_path(app)?, raw).map_err(|e| e.to_string())
}

/// Moteur configuré, avec sa clé ; `None` si rien d'utilisable. `builtin_port` :
/// port du SearXNG intégré s'il tourne.
pub fn search_engine(
    web: &WebConfig,
    tavily_key: Option<String>,
    builtin_port: Option<u16>,
) -> Option<SearchEngine> {
    match web.engine.as_str() {
        "builtin" if web.builtin_enabled => Some(SearchEngine::Searxng {
            url: format!("http://127.0.0.1:{}", builtin_port?),
        }),
        "tavily" => {
            let key = tavily_key
                .or_else(|| web.has_tavily_key.then(|| api_key(TAVILY_KEY_ID)).flatten())?;
            Some(SearchEngine::Tavily { key })
        }
        "searxng" if !web.searxng_url.is_empty() => Some(SearchEngine::Searxng {
            url: web.searxng_url.clone(),
        }),
        _ => None,
    }
}

pub fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

// Toutes les clés vivent dans une seule entrée du Trousseau (un objet JSON
// id → clé), lue une fois par lancement puis gardée en mémoire : macOS ne
// demande l'autorisation qu'une fois, quel que soit le nombre de fournisseurs.
const VAULT_ACCOUNT: &str = "api-keys";
static VAULT: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| e.to_string())
}

fn with_vault<R>(f: impl FnOnce(&mut HashMap<String, String>) -> R) -> Result<R, String> {
    let mut guard = VAULT.lock().unwrap();
    if guard.is_none() {
        let map = match entry(VAULT_ACCOUNT)?.get_password() {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(keyring::Error::NoEntry) => HashMap::new(),
            Err(e) => return Err(format!("Trousseau : {e}")),
        };
        *guard = Some(map);
    }
    Ok(f(guard.as_mut().expect("coffre chargé")))
}

fn persist(map: &HashMap<String, String>) -> Result<(), String> {
    let raw = serde_json::to_string(map).map_err(|e| e.to_string())?;
    entry(VAULT_ACCOUNT)?
        .set_password(&raw)
        .map_err(|e| format!("Trousseau : {e}"))
}

pub fn api_key(id: &str) -> Option<String> {
    if let Some(key) = with_vault(|m| m.get(id).cloned()).ok()? {
        return Some(key);
    }
    // Ancien format (une entrée par fournisseur) : on migre vers le coffre unique.
    let legacy = entry(id).ok()?;
    let key = legacy.get_password().ok()?;
    if set_key(id, &key).is_ok() {
        let _ = legacy.delete_credential();
    }
    Some(key)
}

/// Clé d'un fournisseur configuré, sans toucher au Trousseau s'il n'en a pas.
pub fn key_for(config: &ProviderConfig) -> Option<String> {
    if config.has_key {
        api_key(&config.id)
    } else {
        None
    }
}

fn set_key(id: &str, key: &str) -> Result<(), String> {
    with_vault(|m| {
        m.insert(id.to_string(), key.to_string());
        persist(m)
    })?
}

fn delete_key(id: &str) -> Result<(), String> {
    let _ = entry(id).and_then(|e| match e.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    });
    with_vault(|m| {
        if m.remove(id).is_some() {
            persist(m)
        } else {
            Ok(())
        }
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_urls() {
        assert_eq!(
            normalize_url(" http://localhost:1234/v1/ "),
            "http://localhost:1234/v1"
        );
    }
}
