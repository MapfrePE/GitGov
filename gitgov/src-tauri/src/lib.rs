pub mod audit;
pub mod commands;
pub mod config;
pub mod control_plane;
pub mod git;
pub mod github;
pub mod models;
pub mod outbox;

use outbox::Outbox;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with debug level to see all messages
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .with_thread_ids(false)
        .init();

    let app_data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gitgov");

    let db_path = app_data_dir.join("audit.db");
    let db_path_str = db_path.to_string_lossy().to_string();

    let audit_db = match audit::AuditDatabase::new(&db_path_str) {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("Failed to initialize audit database: {}", e);
            std::process::exit(1);
        }
    };

    let server_url = std::env::var("GITGOV_SERVER_URL").ok();
    let api_key = std::env::var("GITGOV_API_KEY").ok();

    let server_configured = server_url.is_some();

    let outbox = match Outbox::new(&app_data_dir) {
        Ok(o) => {
            let configured = if let Some(url) = server_url {
                o.with_server(url, api_key)
            } else {
                o
            };
            Arc::new(configured)
        }
        Err(e) => {
            eprintln!("Failed to initialize outbox: {}", e);
            std::process::exit(1);
        }
    };

    if !server_configured {
        tracing::warn!("GITGOV_SERVER_URL not configured. Audit events will be stored locally until server is configured.");
    }

    // Shutdown flag for clean worker termination
    let shutdown = Arc::new(AtomicBool::new(false));
    let outbox_clone = Arc::clone(&outbox);
    let worker_handle = outbox_clone.start_background_flush(60);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(audit_db)
        .manage(outbox)
        .setup(move |app| {
            if !server_configured {
                let _ = app.emit("gitgov:server-not-configured", serde_json::json!({
                    "message": "GitGov Server no configurado. Los eventos de auditoría se guardarán localmente hasta que configures el servidor en Settings.",
                    "hint": "Set GITGOV_SERVER_URL environment variable to connect to your GitGov Control Plane."
                }));
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                tracing::info!("Window close requested, signaling worker shutdown");
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::cmd_start_auth,
            commands::cmd_poll_auth,
            commands::cmd_get_current_user,
            commands::cmd_set_current_user,
            commands::cmd_logout,
            commands::cmd_validate_token,
            commands::cmd_get_status,
            commands::cmd_get_file_diff,
            commands::cmd_stage_files,
            commands::cmd_unstage_all,
            commands::cmd_commit,
            commands::cmd_push,
            commands::cmd_list_branches,
            commands::cmd_get_current_branch,
            commands::cmd_create_branch,
            commands::cmd_checkout_branch,
            commands::cmd_get_audit_logs,
            commands::cmd_get_audit_stats,
            commands::cmd_get_my_logs,
            commands::cmd_load_repo_config,
            commands::cmd_validate_repo,
            commands::cmd_validate_branch_name,
            commands::cmd_server_health,
            commands::cmd_server_send_event,
            commands::cmd_server_get_logs,
            commands::cmd_server_get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Signal shutdown and wait for worker to finish
    tracing::info!("Application shutting down, stopping outbox worker...");
    shutdown.store(true, Ordering::Relaxed);

    // Give worker a moment to finish current flush
    if worker_handle.join().is_ok() {
        tracing::info!("Outbox worker stopped cleanly");
    } else {
        tracing::warn!("Outbox worker thread panicked during shutdown");
    }
}
