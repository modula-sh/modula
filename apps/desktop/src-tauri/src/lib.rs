//! Modula desktop shell.
//!
//! On `setup`: link the bundled engine onto PATH and start it (replacing any
//! stale engine from a prior session) before the React app makes its first
//! request. Closing the window hides to the tray with the engine still up;
//! any full quit (tray menu, app menu, Cmd+Q) stops the engine and exits.
//! The engine lifecycle lives in [`engine`]. The React app reaches the engine
//! only through Tauri `invoke`/`Channel` commands ([`grpc`]); the Rust backend
//! is the gRPC client over local IPC.

pub use modula_platform as platform;

mod engine;
mod grpc;
mod update;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine = modula_client::ModulaClient::connect(None).expect("resolve engine IPC endpoint");
    let exit_engine = engine.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        // The flow is driven from Rust (`update`) via the plugin's Rust API;
        // endpoints are set there, only the `pubkey` lives in tauri.conf.json.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::UpdaterState::new())
        .manage(engine.clone())
        .invoke_handler(tauri::generate_handler![
            update::update_status,
            update::install_update,
            update::restart_app,
            grpc::health::engine_health,
            grpc::log::log_stream,
            grpc::workspace::workspace_list,
            grpc::workspace::workspace_create,
            grpc::config::config_get,
            grpc::task::task_list,
            grpc::task::task_create,
            grpc::task::task_upsert,
            grpc::task::task_update,
            grpc::task::task_delete,
            grpc::task::task_reset,
            grpc::task::task_agent_settings,
            grpc::task::task_agent_setting_set,
            grpc::task::task_agent_setting_delete,
            grpc::task::variant_update,
            grpc::label::label_list,
            grpc::label::label_create,
            grpc::label::label_attach,
            grpc::label::label_detach,
            grpc::integration::integration_list,
            grpc::integration::integration_connect,
            grpc::integration::integration_delete,
            grpc::integration::integration_search,
            grpc::integration::integration_fetch,
            grpc::integration::integration_repos,
            grpc::remote::remote_available,
            grpc::remote::remote_status,
            grpc::remote::remote_enable,
            grpc::remote::remote_disable,
            grpc::remote::remote_set_password,
            grpc::remote::remote_begin_pairing,
            grpc::remote::remote_devices,
            grpc::remote::remote_revoke_device,
            grpc::remote::remote_set_device_scope,
            grpc::roadmap::roadmap_set_status,
            grpc::thread::thread_get,
            grpc::thread::thread_append,
            grpc::thread::thread_edit,
            grpc::thread::thread_delete,
            grpc::system::system_tools,
            grpc::snapshot::snapshot_get,
            grpc::conversation::conversation_get,
            grpc::conversation::conversation_create,
            grpc::conversation::conversation_rename,
            grpc::conversation::conversation_delete,
            grpc::conversation::conversation_cancel,
            grpc::conversation::conversation_enqueue,
            grpc::conversation::conversation_dequeue,
            grpc::conversation::conversation_send,
            grpc::conversation::conversation_attach,
            grpc::event::event_watch,
            grpc::run::run_watch,
            grpc::provider::provider_catalog,
            grpc::provider::provider_list,
            grpc::provider::provider_get,
            grpc::provider::provider_create,
            grpc::provider::provider_update,
            grpc::provider::provider_delete,
            grpc::agent::agent_list_running,
            grpc::agent::agent_get,
            grpc::agent::agent_config,
            grpc::agent::agent_skills,
            grpc::agent::agent_create,
            grpc::agent::agent_update,
            grpc::agent::agent_delete,
            grpc::agent::agent_trigger,
            grpc::agent::agent_kill,
            grpc::usage::usage_get,
            grpc::wiki::wiki_tree,
            grpc::wiki::wiki_file,
            grpc::wiki::wiki_create_file,
            grpc::wiki::wiki_write_file,
            grpc::wiki::wiki_create_folder,
            grpc::wiki::wiki_rename,
            grpc::wiki::wiki_delete,
            grpc::project::project_list,
            grpc::project::project_get,
            grpc::project::project_create,
            grpc::project::project_clone,
            grpc::project::project_update,
            grpc::project::project_delete,
            grpc::project::project_diff,
            grpc::project::project_diff_text,
            grpc::project::project_commits,
            grpc::project::project_commit_diff,
            grpc::project::project_stage,
            grpc::project::project_unstage,
            grpc::project::project_task_branches,
            grpc::project::project_repo_branches,
            grpc::diff::variant_diff,
            grpc::diff::variant_pr,
        ])
        .setup(move |app| {
            // In dev (`bash scripts/dev.sh`) the engine is already running under
            // the dev script, so the shell manages nothing.
            if !dev_mode() {
                engine::link_cli();
                if let Err(e) = engine::ensure_running(&engine) {
                    eprintln!("modula: engine failed to start: {e}");
                }
            }
            // Background update poll; surfaces a found update with no user action.
            update::start(app.handle());
            let open = MenuItem::with_id(app, "open", "Open Modula", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Modula", true, Some("CmdOrCtrl+Q"))?;
            let menu = Menu::with_items(app, &[&open, &sep, &quit])?;
            // Monochrome logo as a macOS template image: the OS tints it to
            // match the menu bar (black on light, white on dark). An icon is
            // required or the tray item is invisible once the window hides.
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Modula")
                .icon(tray_icon)
                .icon_as_template(true)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // The native close button always hides to the tray, leaving the app
            // (and engine) running; only an explicit quit (tray or app menu) exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(move |_app, event| {
            // Every quit path (tray, app menu, Cmd+Q) funnels through `Exit`.
            // In dev the dev script owns the engine's lifecycle.
            if matches!(event, tauri::RunEvent::Exit) && !dev_mode() {
                engine::shutdown(&exit_engine);
            }
        });
}

fn dev_mode() -> bool {
    std::env::var_os("MODULA_DEV").is_some()
}
