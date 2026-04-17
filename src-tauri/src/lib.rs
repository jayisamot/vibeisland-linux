// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

pub mod hook;
pub mod ipc;

use tauri::Manager;
use vibeisland_sound as _;
use vibeisland_terminal as _;

use crate::ipc::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("VIBEISLAND_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            ipc::list_sessions,
            ipc::approve,
            ipc::deny,
            ipc::answer_question,
            ipc::focus_terminal,
            ipc::get_config,
            ipc::set_config,
            ipc::list_agents,
            ipc::install_agent,
            ipc::uninstall_agent,
        ])
        .setup(|app| {
            write_overlay_pidfile();
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                match AppState::init(&handle).await {
                    Ok(state) => {
                        handle.manage(state);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "AppState init failed");
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Announce the overlay process to the hook binary. The hook reads
/// this file and falls back to a transparent pass-through (no 5-min
/// block) when the pid is missing or dead.
fn write_overlay_pidfile() {
    let Some(home) = hook::vibeisland_home() else {
        return;
    };
    let _ = std::fs::create_dir_all(&home);
    let pidfile = home.join("overlay.pid");
    let tmp = home.join("overlay.pid.tmp");
    if std::fs::write(&tmp, std::process::id().to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &pidfile);
    }
}
