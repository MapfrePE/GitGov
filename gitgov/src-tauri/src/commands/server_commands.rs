use crate::control_plane::{
    AuditFilter, CombinedEvent, ControlPlaneClient, EventPayload, ServerConfig, ServerStats,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConnectionConfig {
    pub url: String,
    pub api_key: Option<String>,
}

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "message": e.to_string()
    }))
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

#[tauri::command]
pub fn cmd_server_health(config: ServerConnectionConfig) -> Result<bool, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .health_check()
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub fn cmd_server_send_event(
    config: ServerConnectionConfig,
    payload: EventPayload,
) -> Result<String, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    match client.send_event(&payload) {
        Ok(response) => {
            if response.received {
                Ok(response.id)
            } else {
                Err(to_command_error(
                    response
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string()),
                    "EVENT_ERROR",
                ))
            }
        }
        Err(e) => Err(to_command_error(e, "SERVER_ERROR")),
    }
}

#[tauri::command]
pub fn cmd_server_get_logs(
    config: ServerConnectionConfig,
    filter: AuditFilter,
) -> Result<Vec<CombinedEvent>, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .get_logs(&filter)
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub fn cmd_server_get_stats(config: ServerConnectionConfig) -> Result<ServerStats, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .get_stats()
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}
