use crate::control_plane::{
    AuditFilter, CombinedEvent, CommitPipelineCorrelation, ControlPlaneClient, EventPayload,
    JenkinsCorrelationFilter, JiraCorrelateRequest, JiraCorrelateResponse, ServerConfig, ServerStats,
    TicketCoverageQuery, TicketCoverageResponse, JiraTicketDetailResponse,
};
use crate::outbox::Outbox;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConnectionConfig {
    pub url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxSyncResult {
    pub pending_before: usize,
    pub pending_after: usize,
    pub flushed_sent: usize,
    pub flushed_duplicates: usize,
    pub flushed_failed: usize,
}

fn normalize_loopback_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let Ok(mut parsed) = reqwest::Url::parse(trimmed) else {
        return trimmed.to_string();
    };

    if parsed.host_str() == Some("localhost") {
        let _ = parsed.set_host(Some("127.0.0.1"));
    }

    parsed.to_string()
}

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "message": e.to_string()
    }))
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

#[tauri::command]
pub fn cmd_server_sync_outbox(
    config: Option<ServerConnectionConfig>,
    outbox: State<'_, Arc<Outbox>>,
) -> Result<OutboxSyncResult, String> {
    let pending_before = outbox.get_pending_count();

    let normalized_config = config.and_then(|cfg| {
        let url = normalize_loopback_url(&cfg.url);
        if url.trim().is_empty() {
            None
        } else {
            Some(ServerConnectionConfig {
                url,
                api_key: cfg
                    .api_key
                    .and_then(|k| {
                        let trimmed = k.trim().to_string();
                        if trimmed.is_empty() { None } else { Some(trimmed) }
                    }),
            })
        }
    });

    outbox.set_server_config(
        normalized_config.as_ref().map(|c| c.url.clone()),
        normalized_config.as_ref().and_then(|c| c.api_key.clone()),
    );

    let mut flushed_sent = 0;
    let mut flushed_duplicates = 0;
    let mut flushed_failed = 0;

    if normalized_config.is_some() && pending_before > 0 {
        match outbox.flush() {
            Ok(result) => {
                flushed_sent = result.sent;
                flushed_duplicates = result.duplicates;
                flushed_failed = result.failed;
            }
            Err(e) => {
                return Err(to_command_error(e, "OUTBOX_SYNC_ERROR"));
            }
        }
    }

    Ok(OutboxSyncResult {
        pending_before,
        pending_after: outbox.get_pending_count(),
        flushed_sent,
        flushed_duplicates,
        flushed_failed,
    })
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
