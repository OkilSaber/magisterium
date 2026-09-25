use crate::config::{self, ProviderConfig};
use crate::history;
use crate::orchestrator::{self, RunConfig, RunEvent};
use crate::providers::{self, ModelInfo, ProviderInfo};
use crate::searxng::{self, SearxngState};
use crate::tools::{self, ModelTools};
use crate::usage;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

/// Sessions en cours, pour pouvoir les annuler.
#[derive(Default)]
pub struct Runs(Mutex<HashMap<String, CancellationToken>>);

#[tauri::command]
pub async fn list_providers(app: AppHandle) -> Vec<ProviderInfo> {
    providers::detect_all(&app).await
}

#[tauri::command]
pub async fn refresh_provider(app: AppHandle, provider: String) -> Result<ProviderInfo, String> {
    providers::detect_one(&app, &provider).await
}

#[tauri::command]
pub fn get_provider_configs(app: AppHandle) -> Result<Vec<ProviderConfig>, String> {
    config::load(&app)
}

/// `api_key` : absent = garder la clé actuelle, vide = la supprimer.
#[tauri::command]
pub fn save_provider(
    app: AppHandle,
    provider: ProviderConfig,
    api_key: Option<String>,
) -> Result<ProviderConfig, String> {
    config::upsert(&app, provider, api_key)
}

#[tauri::command]
pub fn delete_provider(app: AppHandle, id: String) -> Result<(), String> {
    config::remove(&app, &id)
}

/// Vérifie la connexion avant d'enregistrer : renvoie les modèles trouvés.
/// Sans clé fournie, on essaie celle déjà enregistrée pour ce fournisseur.
#[tauri::command]
pub async fn test_provider(
    provider: ProviderConfig,
    api_key: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let key = api_key
        .filter(|k| !k.trim().is_empty())
        .or_else(|| config::key_for(&provider));
    providers::list_api_models(provider.kind, &provider.base_url, key.as_deref()).await
}

#[tauri::command]
pub fn start_run(
    mut config: RunConfig,
    on_event: Channel<RunEvent>,
    app: AppHandle,
    runs: State<'_, Runs>,
) -> Result<String, String> {
    if config.prompt.trim().is_empty() {
        return Err("Le prompt est vide".into());
    }
    if config.agents.is_empty() {
        return Err("Choisis au moins une IA".into());
    }
    // Sans dossier choisi, les agents travaillent dans un dossier temporaire.
    let has_workspace = !config.workdir.trim().is_empty();
    if !has_workspace {
        let scratch = std::env::temp_dir().join("magisterium");
        std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
        config.workdir = scratch.to_string_lossy().into_owned();
    }
    if !Path::new(&config.workdir).is_dir() {
        return Err(format!(
            "Dossier de travail introuvable : {}",
            config.workdir
        ));
    }

    // Résout chaque fournisseur (et sa clé API) avant de lancer quoi que ce soit.
    let mut backends = HashMap::new();
    for agent in config.agents.iter().chain(config.synthesizer.as_ref()) {
        if !backends.contains_key(&agent.provider) {
            backends.insert(
                agent.provider.clone(),
                providers::resolve(&app, &agent.provider)?,
            );
        }
    }

    // Outils des modèles API : le web s'il est activé, et la lecture du dossier de
    // travail si l'utilisateur en a choisi un (pas le dossier temporaire).
    let tools = ModelTools {
        web: config.web,
        engine: if config.web {
            config::search_engine(
                &config::load_web(&app),
                None,
                app.state::<SearxngState>().port(),
            )
        } else {
            None
        },
        workspace: has_workspace.then(|| std::path::PathBuf::from(&config.workdir)),
        today: config.today.clone(),
    };
    let web = (!tools.is_empty()).then_some(tools);

    let run_id = uuid::Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    runs.0.lock().unwrap().insert(run_id.clone(), token.clone());

    let id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        let emit = move |event: RunEvent| {
            let _ = on_event.send(event);
        };
        // Abandonner la future tue les processus enfants (kill_on_drop).
        let cancelled = tokio::select! {
            _ = orchestrator::execute(config, &backends, web.as_ref(), &emit) => false,
            _ = token.cancelled() => true,
        };
        emit(RunEvent::Finished { cancelled });
        app.state::<Runs>().0.lock().unwrap().remove(&id);
    });

    Ok(run_id)
}

#[tauri::command]
pub fn cancel_run(run_id: String, runs: State<'_, Runs>) {
    if let Some(token) = runs.0.lock().unwrap().get(&run_id) {
        token.cancel();
    }
}

#[tauri::command]
pub fn list_conversations(app: AppHandle) -> Result<Vec<history::Summary>, String> {
    history::list(&app)
}

#[tauri::command]
pub fn load_conversation(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    history::load(&app, &id)
}

#[tauri::command]
pub fn save_conversation(app: AppHandle, conversation: serde_json::Value) -> Result<(), String> {
    history::save(&app, &conversation)
}

#[tauri::command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    history::delete(&app, &id)
}

#[tauri::command]
pub fn get_web_config(app: AppHandle) -> config::WebConfig {
    config::load_web(&app)
}

#[tauri::command]
pub fn save_web_config(
    app: AppHandle,
    web: config::WebConfig,
    tavily_key: Option<String>,
) -> Result<config::WebConfig, String> {
    config::save_web(&app, web, tavily_key)
}

/// Lance une recherche d'essai avec la configuration saisie (avant enregistrement).
#[tauri::command]
pub async fn test_web_search(
    app: AppHandle,
    web: config::WebConfig,
    tavily_key: Option<String>,
) -> Result<usize, String> {
    let key = tavily_key.filter(|k| !k.trim().is_empty());
    let web = config::WebConfig {
        has_tavily_key: config::load_web(&app).has_tavily_key,
        ..web
    };
    let port = app.state::<SearxngState>().port();
    let engine = config::search_engine(&web, key, port).ok_or("Aucun moteur configuré")?;
    Ok(tools::search(&engine, "Louis de Funès").await?.len())
}

fn searxng_paths(app: &AppHandle) -> Result<searxng::Paths, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(searxng::Paths::new(dir.join("searxng")))
}

#[derive(serde::Serialize, Clone)]
pub struct SearxngStatus {
    installed: bool,
    enabled: bool,
    installing: bool,
    starting: bool,
    port: Option<u16>,
    error: Option<String>,
    size_bytes: u64,
}

fn searxng_status_of(app: &AppHandle) -> Result<SearxngStatus, String> {
    let paths = searxng_paths(app)?;
    let state = app.state::<SearxngState>();
    let web = config::load_web(app);
    let installed = paths.installed();
    let installing = *state.installing.lock().unwrap();
    let starting = *state.starting.lock().unwrap();
    let error = state.last_error.lock().unwrap().clone();
    Ok(SearxngStatus {
        installed,
        enabled: web.builtin_enabled,
        installing,
        starting,
        port: state.port(),
        error,
        size_bytes: if installed { paths.size_bytes() } else { 0 },
    })
}

fn emit_status(app: &AppHandle) {
    if let Ok(status) = searxng_status_of(app) {
        let _ = app.emit("searxng-status", status);
    }
}

/// Démarre le SearXNG intégré s'il ne tourne pas déjà.
pub async fn start_builtin(app: &AppHandle) {
    let state = app.state::<SearxngState>();
    if state.port().is_some() || std::mem::replace(&mut *state.starting.lock().unwrap(), true) {
        return;
    }
    emit_status(app);
    let result = match searxng_paths(app) {
        Ok(paths) => searxng::Server::start(&paths).await,
        Err(e) => Err(e),
    };
    match result {
        Ok(server) => {
            *state.server.lock().unwrap() = Some(server);
            *state.last_error.lock().unwrap() = None;
        }
        Err(e) => *state.last_error.lock().unwrap() = Some(e),
    }
    *state.starting.lock().unwrap() = false;
    emit_status(app);
}

async fn stop_builtin(app: &AppHandle) {
    let server = app.state::<SearxngState>().server.lock().unwrap().take();
    if let Some(server) = server {
        server.stop().await;
    }
    emit_status(app);
}

#[tauri::command]
pub fn searxng_status(app: AppHandle) -> Result<SearxngStatus, String> {
    searxng_status_of(&app)
}

/// Installe SearXNG (progression via l'événement `searxng-progress`), puis l'active.
#[tauri::command]
pub async fn searxng_install(app: AppHandle) -> Result<SearxngStatus, String> {
    let state = app.state::<SearxngState>();
    if std::mem::replace(&mut *state.installing.lock().unwrap(), true) {
        return Err("Installation déjà en cours".into());
    }
    *state.last_error.lock().unwrap() = None;
    emit_status(&app);

    let emitter = app.clone();
    let on = move |p: searxng::Progress| {
        let _ = emitter.emit("searxng-progress", p);
    };
    let result = searxng::install(&searxng_paths(&app)?, &on).await;
    *state.installing.lock().unwrap() = false;

    match result {
        Ok(()) => {
            let web = config::WebConfig {
                engine: "builtin".into(),
                builtin_enabled: true,
                ..config::load_web(&app)
            };
            config::write_web(&app, &web)?;
            start_builtin(&app).await;
        }
        Err(e) => {
            *state.last_error.lock().unwrap() = Some(e.clone());
            emit_status(&app);
            return Err(e);
        }
    }
    searxng_status_of(&app)
}

#[tauri::command]
pub async fn searxng_set_enabled(app: AppHandle, enabled: bool) -> Result<SearxngStatus, String> {
    let web = config::WebConfig {
        builtin_enabled: enabled,
        ..config::load_web(&app)
    };
    config::write_web(&app, &web)?;
    if enabled {
        start_builtin(&app).await;
    } else {
        stop_builtin(&app).await;
    }
    searxng_status_of(&app)
}

#[tauri::command]
pub async fn searxng_uninstall(app: AppHandle) -> Result<SearxngStatus, String> {
    stop_builtin(&app).await;
    let paths = searxng_paths(&app)?;
    paths.remove()?;
    let web = config::WebConfig {
        builtin_enabled: false,
        ..config::load_web(&app)
    };
    config::write_web(&app, &web)?;
    emit_status(&app);
    searxng_status_of(&app)
}

/// Limites d'usage de tous les fournisseurs qui les exposent.
#[tauri::command]
pub async fn get_usage(app: AppHandle) -> Result<Vec<usage::ProviderUsage>, String> {
    let configs = config::load(&app)?;
    let web = config::load_web(&app);
    let tavily_key = (web.engine == "tavily" && web.has_tavily_key)
        .then(|| config::search_engine(&web, None, None))
        .flatten()
        .and_then(|e| match e {
            tools::SearchEngine::Tavily { key } => Some(key),
            _ => None,
        });
    let searxng = web.engine == "builtin" && searxng_paths(&app)?.installed();
    Ok(usage::collect(&configs, tavily_key, searxng).await)
}
