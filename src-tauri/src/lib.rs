mod commands;
mod config;
mod env;
mod history;
mod orchestrator;
mod prompts;
mod providers;
mod searxng;
mod tools;
mod usage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env::init_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Vrai verre dépoli macOS derrière la webview : le bureau transparaît.
            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                use window_vibrancy::{
                    apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
                };
                if let Some(window) = app.get_webview_window("main") {
                    let _ = apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::UnderWindowBackground,
                        Some(NSVisualEffectState::Active),
                        None,
                    );
                }
            }

            // SearXNG intégré : démarré seulement s'il a été installé ET activé.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Manager;
                let web = config::load_web(&handle);
                let installed = handle
                    .path()
                    .app_data_dir()
                    .map(|d| searxng::Paths::new(d.join("searxng")).installed())
                    .unwrap_or(false);
                if installed && web.engine == "builtin" && web.builtin_enabled {
                    commands::start_builtin(&handle).await;
                }
            });
            Ok(())
        })
        .manage(commands::Runs::default())
        .manage(searxng::SearxngState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::refresh_provider,
            commands::get_provider_configs,
            commands::save_provider,
            commands::delete_provider,
            commands::test_provider,
            commands::get_web_config,
            commands::save_web_config,
            commands::test_web_search,
            commands::searxng_status,
            commands::searxng_install,
            commands::searxng_set_enabled,
            commands::searxng_uninstall,
            commands::get_usage,
            commands::start_run,
            commands::cancel_run,
            commands::list_conversations,
            commands::load_conversation,
            commands::save_conversation,
            commands::delete_conversation,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Arrête SearXNG avec l'app : aucun processus ne reste derrière.
            if let tauri::RunEvent::Exit = event {
                use tauri::Manager;
                app.state::<searxng::SearxngState>().kill_now();
            }
        });
}
