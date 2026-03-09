use crate::control_plane::{
    AcceptOrgInvitationRequest, AcceptOrgInvitationResponse, ApiKeyInfo, ApiKeyResponse,
    AuditFilter, ChatAskRequest, ChatAskResponse, CombinedEvent, CommitPipelineCorrelation,
    ControlPlaneClient, CreateOrgInvitationRequest, CreateOrgInvitationResponse, CreateOrgRequest,
    CreateOrgResponse, CreateOrgUserRequest, CreateOrgUserResponse, DailyActivityFilter,
    DailyActivityPoint, EventPayload, ExportLogEntry, ExportResponse, FeatureRequestCreated,
    FeatureRequestInput, JenkinsCorrelationFilter, JiraCorrelateRequest, JiraCorrelateResponse,
    JiraTicketDetailResponse, MeResponse, OrgInvitation, OrgInvitationsResponse, OrgUser,
    OrgUsersResponse, PolicyCheckResponse, PolicyHistoryEntry, PolicyResponse,
    PrMergeEvidenceEntry, PrMergeEvidenceFilter, ResendOrgInvitationRequest, RevokeApiKeyResponse,
    ServerConfig, ServerStats, TeamOverviewResponse, TeamReposResponse, TicketCoverageQuery,
    TicketCoverageResponse,
};
use crate::models::GitGovConfig;
use crate::outbox::Outbox;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Emitter, State};

const KEYRING_SERVICE: &str = "gitgov";
const CONTROL_PLANE_API_KEY_ACCOUNT: &str = "control_plane_api_key";
const LOCAL_PIN_ACCOUNT: &str = "local_pin";

/// Monotonic generation counter for SSE connections.
/// Each new connection increments the counter; a stream loop only
/// continues while its local generation matches the current value.
/// This prevents stale streams from surviving a quick disconnect→reconnect.
pub struct SseGeneration(pub Arc<AtomicU64>);

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

    // Control Plane config should always be a base URL only (scheme + host + optional port).
    // Strip path/query/fragment to avoid posting outbox events to /docs/events or /health/events.
    parsed.set_path("/");
    parsed.set_query(None);
    parsed.set_fragment(None);

    let mut base = parsed.to_string();
    while base.ends_with('/') {
        base.pop();
    }
    base
}

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "message": e.to_string()
    }))
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

async fn run_blocking_command<T, F>(task_name: &'static str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| {
        to_command_error(
            format!("{}_THREAD_JOIN_ERROR: {}", task_name, e),
            "SERVER_ERROR",
        )
    })?
}

fn is_keyring_no_entry_error(error: &keyring::Error) -> bool {
    match error {
        #[allow(unreachable_patterns)]
        keyring::Error::NoEntry => true,
        _ => {
            let msg = error.to_string().to_ascii_lowercase();
            msg.contains("no matching entry found")
                || msg.contains("no entry")
                || msg.contains("credential not found")
        }
    }
}

fn load_secure_secret(account: &'static str) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, account)
        .map_err(|e| to_command_error(e, "SECURE_STORAGE_ERROR"))?;
    match entry.get_password() {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(e) if is_keyring_no_entry_error(&e) => Ok(None),
        Err(e) => Err(to_command_error(e, "SECURE_STORAGE_ERROR")),
    }
}

fn save_secure_secret(account: &'static str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, account)
        .map_err(|e| to_command_error(e, "SECURE_STORAGE_ERROR"))?;
    entry
        .set_password(value)
        .map_err(|e| to_command_error(e, "SECURE_STORAGE_ERROR"))
}

fn clear_secure_secret(account: &'static str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, account)
        .map_err(|e| to_command_error(e, "SECURE_STORAGE_ERROR"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(e) if is_keyring_no_entry_error(&e) => Ok(()),
        Err(e) => Err(to_command_error(e, "SECURE_STORAGE_ERROR")),
    }
}

fn is_valid_local_pin(pin: &str) -> bool {
    let len = pin.len();
    (4..=6).contains(&len) && pin.chars().all(|c| c.is_ascii_digit())
}

#[tauri::command]
pub async fn cmd_cp_get_api_key() -> Result<Option<String>, String> {
    run_blocking_command("CP_GET_API_KEY", || {
        load_secure_secret(CONTROL_PLANE_API_KEY_ACCOUNT)
    })
    .await
}

#[tauri::command]
pub async fn cmd_cp_set_api_key(api_key: String) -> Result<(), String> {
    run_blocking_command("CP_SET_API_KEY", move || {
        let normalized = api_key.trim().to_string();
        if normalized.is_empty() {
            return Err(to_command_error(
                "API key vacia no permitida",
                "VALIDATION_ERROR",
            ));
        }
        save_secure_secret(CONTROL_PLANE_API_KEY_ACCOUNT, &normalized)
    })
    .await
}

#[tauri::command]
pub async fn cmd_cp_clear_api_key() -> Result<(), String> {
    run_blocking_command("CP_CLEAR_API_KEY", || {
        clear_secure_secret(CONTROL_PLANE_API_KEY_ACCOUNT)
    })
    .await
}

#[tauri::command]
pub async fn cmd_pin_get() -> Result<Option<String>, String> {
    run_blocking_command("PIN_GET", || load_secure_secret(LOCAL_PIN_ACCOUNT)).await
}

#[tauri::command]
pub async fn cmd_pin_set(pin: String) -> Result<(), String> {
    run_blocking_command("PIN_SET", move || {
        let normalized = pin.trim().to_string();
        if !is_valid_local_pin(&normalized) {
            return Err(to_command_error(
                "PIN invalido. Debe tener entre 4 y 6 digitos.",
                "VALIDATION_ERROR",
            ));
        }
        save_secure_secret(LOCAL_PIN_ACCOUNT, &normalized)
    })
    .await
}

#[tauri::command]
pub async fn cmd_pin_clear() -> Result<(), String> {
    run_blocking_command("PIN_CLEAR", || clear_secure_secret(LOCAL_PIN_ACCOUNT)).await
}

#[tauri::command]
pub async fn cmd_server_sync_outbox(
    config: Option<ServerConnectionConfig>,
    outbox: State<'_, Arc<Outbox>>,
) -> Result<OutboxSyncResult, String> {
    let outbox = outbox.inner().clone();
    run_blocking_command("OUTBOX_SYNC", move || {
        let pending_before = outbox.get_pending_count();

        let normalized_config = config.and_then(|cfg| {
            let url = normalize_loopback_url(&cfg.url);
            if url.trim().is_empty() {
                None
            } else {
                Some(ServerConnectionConfig {
                    url,
                    api_key: cfg.api_key.and_then(|k| {
                        let trimmed = k.trim().to_string();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        }
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
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_health(config: ServerConnectionConfig) -> Result<bool, String> {
    run_blocking_command("HEALTH_CHECK", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .health_check()
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_send_event(
    config: ServerConnectionConfig,
    payload: EventPayload,
) -> Result<String, String> {
    run_blocking_command("SEND_EVENT", move || {
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
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_logs(
    config: ServerConnectionConfig,
    filter: AuditFilter,
) -> Result<Vec<CombinedEvent>, String> {
    run_blocking_command("GET_LOGS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_logs(&filter)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_stats(config: ServerConnectionConfig) -> Result<ServerStats, String> {
    run_blocking_command("GET_STATS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_stats()
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_daily_activity(
    config: ServerConnectionConfig,
    filter: DailyActivityFilter,
) -> Result<Vec<DailyActivityPoint>, String> {
    run_blocking_command("GET_DAILY_ACTIVITY", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_daily_activity(&filter)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_team_overview(
    config: ServerConnectionConfig,
    org_name: Option<String>,
    status: Option<String>,
    days: Option<i64>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<TeamOverviewResponse, String> {
    run_blocking_command("GET_TEAM_OVERVIEW", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_team_overview(
                org_name.as_deref(),
                status.as_deref(),
                days.unwrap_or(30),
                limit.unwrap_or(50),
                offset.unwrap_or(0),
            )
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_team_repos(
    config: ServerConnectionConfig,
    org_name: Option<String>,
    days: Option<i64>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<TeamReposResponse, String> {
    run_blocking_command("GET_TEAM_REPOS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_team_repos(
                org_name.as_deref(),
                days.unwrap_or(30),
                limit.unwrap_or(50),
                offset.unwrap_or(0),
            )
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_jenkins_correlations(
    config: ServerConnectionConfig,
    filter: JenkinsCorrelationFilter,
) -> Result<Vec<CommitPipelineCorrelation>, String> {
    run_blocking_command("GET_JENKINS_CORRELATIONS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_jenkins_correlations(&filter)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_pr_merges(
    config: ServerConnectionConfig,
    filter: PrMergeEvidenceFilter,
) -> Result<Vec<PrMergeEvidenceEntry>, String> {
    run_blocking_command("GET_PR_MERGES", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_pr_merges(&filter)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_jira_ticket_coverage(
    config: ServerConnectionConfig,
    query: TicketCoverageQuery,
) -> Result<TicketCoverageResponse, String> {
    run_blocking_command("GET_JIRA_TICKET_COVERAGE", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_jira_ticket_coverage(&query)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_correlate_jira_tickets(
    config: ServerConnectionConfig,
    request: JiraCorrelateRequest,
) -> Result<JiraCorrelateResponse, String> {
    run_blocking_command("CORRELATE_JIRA_TICKETS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .correlate_jira_tickets(&request)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_jira_ticket_detail(
    config: ServerConnectionConfig,
    ticket_id: String,
) -> Result<JiraTicketDetailResponse, String> {
    run_blocking_command("GET_JIRA_TICKET_DETAIL", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_jira_ticket_detail(&ticket_id)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_me(config: ServerConnectionConfig) -> Result<MeResponse, String> {
    run_blocking_command("GET_ME", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_me()
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_create_org(
    config: ServerConnectionConfig,
    payload: CreateOrgRequest,
) -> Result<CreateOrgResponse, String> {
    run_blocking_command("CREATE_ORG", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .create_org(&payload)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_create_org_user(
    config: ServerConnectionConfig,
    payload: CreateOrgUserRequest,
) -> Result<CreateOrgUserResponse, String> {
    run_blocking_command("CREATE_ORG_USER", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .create_org_user(&payload)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_list_org_users(
    config: ServerConnectionConfig,
    org_name: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<OrgUsersResponse, String> {
    run_blocking_command("LIST_ORG_USERS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .list_org_users(
                org_name.as_deref(),
                status.as_deref(),
                limit.unwrap_or(50),
                offset.unwrap_or(0),
            )
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_update_org_user_status(
    config: ServerConnectionConfig,
    user_id: String,
    status: String,
) -> Result<OrgUser, String> {
    run_blocking_command("UPDATE_ORG_USER_STATUS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .update_org_user_status(&user_id, &status)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_create_api_key_for_org_user(
    config: ServerConnectionConfig,
    user_id: String,
) -> Result<ApiKeyResponse, String> {
    run_blocking_command("CREATE_ORG_USER_API_KEY", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .create_api_key_for_org_user(&user_id)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_create_org_invitation(
    config: ServerConnectionConfig,
    payload: CreateOrgInvitationRequest,
) -> Result<CreateOrgInvitationResponse, String> {
    run_blocking_command("CREATE_ORG_INVITATION", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .create_org_invitation(&payload)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_list_org_invitations(
    config: ServerConnectionConfig,
    org_name: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<OrgInvitationsResponse, String> {
    run_blocking_command("LIST_ORG_INVITATIONS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .list_org_invitations(
                org_name.as_deref(),
                status.as_deref(),
                limit.unwrap_or(50),
                offset.unwrap_or(0),
            )
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_resend_org_invitation(
    config: ServerConnectionConfig,
    invitation_id: String,
    expires_in_days: Option<i64>,
) -> Result<CreateOrgInvitationResponse, String> {
    run_blocking_command("RESEND_ORG_INVITATION", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .resend_org_invitation(
                &invitation_id,
                &ResendOrgInvitationRequest { expires_in_days },
            )
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_revoke_org_invitation(
    config: ServerConnectionConfig,
    invitation_id: String,
) -> Result<OrgInvitation, String> {
    run_blocking_command("REVOKE_ORG_INVITATION", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .revoke_org_invitation(&invitation_id)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_preview_org_invitation(
    config: ServerConnectionConfig,
    token: String,
) -> Result<OrgInvitation, String> {
    run_blocking_command("PREVIEW_ORG_INVITATION", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .preview_org_invitation(&token)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_accept_org_invitation(
    config: ServerConnectionConfig,
    token: String,
    login: Option<String>,
) -> Result<AcceptOrgInvitationResponse, String> {
    run_blocking_command("ACCEPT_ORG_INVITATION", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .accept_org_invitation(&AcceptOrgInvitationRequest { token, login })
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_list_api_keys(
    config: ServerConnectionConfig,
) -> Result<Vec<ApiKeyInfo>, String> {
    run_blocking_command("LIST_API_KEYS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .list_api_keys()
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_revoke_api_key(
    config: ServerConnectionConfig,
    key_id: String,
) -> Result<RevokeApiKeyResponse, String> {
    run_blocking_command("REVOKE_API_KEY", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .revoke_api_key(&key_id)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_export(
    config: ServerConnectionConfig,
    export_type: String,
    start_date: Option<i64>,
    end_date: Option<i64>,
    org_name: Option<String>,
) -> Result<ExportResponse, String> {
    run_blocking_command("EXPORT_EVENTS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .export_events(&export_type, start_date, end_date, org_name.as_deref())
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_list_exports(
    config: ServerConnectionConfig,
) -> Result<Vec<ExportLogEntry>, String> {
    run_blocking_command("LIST_EXPORTS", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });

        client
            .list_exports()
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

// ── Conversational Chat ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn cmd_server_chat_ask(
    config: ServerConnectionConfig,
    request: ChatAskRequest,
) -> Result<ChatAskResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client.chat_ask(&request)
    })
    .await
    .map_err(|e| to_command_error(format!("CHAT_THREAD_JOIN_ERROR: {}", e), "SERVER_ERROR"))?
    .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub async fn cmd_server_create_feature_request(
    config: ServerConnectionConfig,
    input: FeatureRequestInput,
) -> Result<FeatureRequestCreated, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client.create_feature_request(&input)
    })
    .await
    .map_err(|e| {
        to_command_error(
            format!("FEATURE_REQUEST_THREAD_JOIN_ERROR: {}", e),
            "SERVER_ERROR",
        )
    })?
    .map_err(|e| to_command_error(e, "SERVER_ERROR"))
}

#[tauri::command]
pub async fn cmd_server_get_policy(
    config: ServerConnectionConfig,
    repo_name: String,
) -> Result<Option<PolicyResponse>, String> {
    run_blocking_command("GET_POLICY", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_policy(&repo_name)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_override_policy(
    config: ServerConnectionConfig,
    repo_name: String,
    policy_config: GitGovConfig,
) -> Result<PolicyResponse, String> {
    run_blocking_command("OVERRIDE_POLICY", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .override_policy(&repo_name, &policy_config)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_get_policy_history(
    config: ServerConnectionConfig,
    repo_name: String,
) -> Result<Vec<PolicyHistoryEntry>, String> {
    run_blocking_command("GET_POLICY_HISTORY", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .get_policy_history(&repo_name)
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

#[tauri::command]
pub async fn cmd_server_policy_check(
    config: ServerConnectionConfig,
    repo: String,
    branch: String,
    user_login: Option<String>,
) -> Result<PolicyCheckResponse, String> {
    run_blocking_command("POLICY_CHECK", move || {
        let client = ControlPlaneClient::new(ServerConfig {
            url: config.url,
            api_key: config.api_key,
        });
        client
            .policy_check(&repo, &branch, user_login.as_deref())
            .map_err(|e| to_command_error(e, "SERVER_ERROR"))
    })
    .await
}

/// Connects to the server's SSE endpoint and forwards notifications as Tauri events.
/// Uses a generation counter so that `cmd_server_sse_disconnect` (or a newer connect)
/// invalidates any stale stream without race conditions.
#[tauri::command]
pub async fn cmd_server_sse_connect(
    config: ServerConnectionConfig,
    app: tauri::AppHandle,
    gen: State<'_, SseGeneration>,
) -> Result<(), String> {
    // Bump generation: any older stream loop will see a mismatch and exit.
    let my_gen = gen.0.fetch_add(1, Ordering::SeqCst) + 1;
    let gen_counter = Arc::clone(&gen.0);

    let url = format!("{}/sse", config.url);
    let client = reqwest::Client::new();
    let mut request = client.get(&url);
    if let Some(ref api_key) = config.api_key {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = request
        .send()
        .await
        .map_err(|e| to_command_error(e, "SSE_CONNECT_ERROR"))?;

    if !response.status().is_success() {
        return Err(to_command_error(
            format!("SSE connection failed: {}", response.status()),
            "SSE_CONNECT_ERROR",
        ));
    }

    let _ = app.emit("gitgov:sse-connected", serde_json::json!({}));

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        // If generation changed, a newer connection or disconnect invalidated us
        if gen_counter.load(Ordering::SeqCst) != my_gen {
            tracing::info!(my_gen, "SSE stream superseded by newer generation");
            break;
        }

        match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                buffer.push_str(&text);

                // Parse SSE protocol — handles both \n\n and \r\n\r\n
                while let Some(pos) = find_sse_boundary(&buffer) {
                    let boundary_len = if buffer[pos..].starts_with("\r\n\r\n") {
                        4
                    } else {
                        2
                    };
                    let message = buffer[..pos].to_string();
                    buffer = buffer[pos + boundary_len..].to_string();

                    for line in message.lines() {
                        // Accept both "data: value" and "data:value"
                        let data = line
                            .strip_prefix("data: ")
                            .or_else(|| line.strip_prefix("data:"));
                        if let Some(data) = data {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                                let _ = app.emit("gitgov:sse-event", parsed);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "SSE stream error");
                break;
            }
        }
    }

    // Only emit disconnected if we are still the active generation
    if gen_counter.load(Ordering::SeqCst) == my_gen {
        let _ = app.emit("gitgov:sse-disconnected", serde_json::json!({}));
    }
    Ok(())
}

/// Signals any running SSE stream to stop by bumping the generation counter.
#[tauri::command]
pub async fn cmd_server_sse_disconnect(gen: State<'_, SseGeneration>) -> Result<(), String> {
    gen.0.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Find the next SSE message boundary (\n\n or \r\n\r\n).
fn find_sse_boundary(buf: &str) -> Option<usize> {
    // Check \r\n\r\n first (more specific), then \n\n
    if let Some(pos) = buf.find("\r\n\r\n") {
        // But only if there isn't an earlier \n\n
        if let Some(pos2) = buf.find("\n\n") {
            return Some(pos.min(pos2));
        }
        return Some(pos);
    }
    buf.find("\n\n")
}
