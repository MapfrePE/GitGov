use crate::control_plane::{
    AuditFilter, CombinedEvent, CommitPipelineCorrelation, ControlPlaneClient, EventPayload,
    JenkinsCorrelationFilter, JiraCorrelateRequest, JiraCorrelateResponse, ServerConfig, ServerStats,
    TicketCoverageQuery, TicketCoverageResponse, JiraTicketDetailResponse,
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

#[tauri::command]
pub fn cmd_server_get_jenkins_correlations(
    config: ServerConnectionConfig,
    filter: JenkinsCorrelationFilter,
) -> Result<Vec<CommitPipelineCorrelation>, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .get_jenkins_correlations(&filter)
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub fn cmd_server_get_jira_ticket_coverage(
    config: ServerConnectionConfig,
    query: TicketCoverageQuery,
) -> Result<TicketCoverageResponse, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .get_jira_ticket_coverage(&query)
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub fn cmd_server_correlate_jira_tickets(
    config: ServerConnectionConfig,
    request: JiraCorrelateRequest,
) -> Result<JiraCorrelateResponse, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .correlate_jira_tickets(&request)
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub fn cmd_server_get_jira_ticket_detail(
    config: ServerConnectionConfig,
    ticket_id: String,
) -> Result<JiraTicketDetailResponse, String> {
    let client = ControlPlaneClient::new(ServerConfig {
        url: config.url,
        api_key: config.api_key,
    });

    client
        .get_jira_ticket_detail(&ticket_id)
        .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}
