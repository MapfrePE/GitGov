pub mod audit;
pub mod commands;
pub mod config;
pub mod control_plane;
pub mod git;
pub mod github;
pub mod models;
pub mod outbox;

use outbox::Outbox;
use std::sync::Arc;
use tauri::Emitter;

fn normalize_loopback_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let Ok(mut parsed) = reqwest::Url::parse(trimmed) else {
        return trimmed.to_string();
    };

    if parsed.host_str() == Some("localhost") {
        if parsed.set_host(Some("127.0.0.1")).is_ok() {
            return parsed.to_string();
        }
    }

    trimmed.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env file if present
    dotenvy::dotenv().ok();

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

    let server_url = std::env::var("GITGOV_SERVER_URL")
        .ok()
        .map(|u| normalize_loopback_url(&u));
    let api_key = std::env::var("GITGOV_API_KEY").ok();

    let server_configured = server_url.is_some();

    if server_configured {
        tracing::info!(
            server_url = ?server_url,
            has_api_key = api_key.is_some(),
            "GitGov Server configured from environment"
        );
    }

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

    let outbox_clone = Arc::clone(&outbox);
    let worker_handle = outbox_clone.start_background_flush(60);

    // Heartbeat timer — fires every 10 min to track last_seen on the Control Plane.
    // Reads current_user.json from disk (same location as auth_commands.rs uses).
    let outbox_hb = Arc::clone(&outbox);
    std::thread::spawn(move || {
        const HEARTBEAT_INTERVAL: std::time::Duration =
            std::time::Duration::from_secs(600); // 10 minutes
        loop {
            std::thread::sleep(HEARTBEAT_INTERVAL);
            let user_file = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("gitgov")
                .join("current_user.json");
            let login_opt = std::fs::read_to_string(&user_file)
                .ok()
                .and_then(|json| {
                    serde_json::from_str::<models::AuthenticatedUser>(&json)
                        .ok()
                        .map(|u| u.login)
                });
            if let Some(user_login) = login_opt {
                let event = outbox::OutboxEvent::new(
                    "heartbeat".to_string(),
                    user_login.clone(),
                    None,
                    models::AuditStatus::Success,
                )
                .with_metadata(serde_json::json!({
                    "device": {
                        "hostname": std::env::var("COMPUTERNAME")
                            .or_else(|_| std::env::var("HOSTNAME"))
                            .ok(),
                        "os": std::env::consts::OS,
                        "arch": std::env::consts::ARCH,
                    }
                }));
                let _ = outbox_hb.add(event);
                tracing::debug!(user_login = %user_login, "Heartbeat event queued");
            }
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
        .on_window_event(|_window, event| {
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
            commands::cmd_apply_ignore_rules,
            commands::cmd_read_gitgovignore,
            commands::cmd_remove_ignore_rules,
            commands::cmd_stage_files,
            commands::cmd_unstage_all,
            commands::cmd_unstage_files,
            commands::cmd_commit,
            commands::cmd_push,
            commands::cmd_list_branches,
            commands::cmd_get_current_branch,
            commands::cmd_get_branch_sync_status,
            commands::cmd_create_branch,
            commands::cmd_checkout_branch,
            commands::cmd_get_audit_logs,
            commands::cmd_get_audit_stats,
            commands::cmd_get_my_logs,
            commands::cmd_load_repo_config,
            commands::cmd_validate_repo,
            commands::cmd_validate_branch_name,
            commands::cmd_server_sync_outbox,
            commands::cmd_server_health,
            commands::cmd_server_send_event,
            commands::cmd_server_get_logs,
            commands::cmd_server_get_stats,
            commands::cmd_server_get_daily_activity,
            commands::cmd_server_get_jenkins_correlations,
            commands::cmd_server_get_pr_merges,
            commands::cmd_server_get_jira_ticket_coverage,
            commands::cmd_server_correlate_jira_tickets,
            commands::cmd_server_get_jira_ticket_detail,
            commands::cmd_server_get_me,
            commands::cmd_server_list_api_keys,
            commands::cmd_server_revoke_api_key,
            commands::cmd_server_export,
            commands::cmd_server_list_exports,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Signal shutdown and wait for worker to finish
    tracing::info!("Application shutting down, stopping outbox worker...");
    outbox_clone.signal_shutdown();

    // Give worker a moment to finish current flush
    if worker_handle.join().is_ok() {
        tracing::info!("Outbox worker stopped cleanly");
    } else {
        tracing::warn!("Outbox worker thread panicked during shutdown");
    }
}
