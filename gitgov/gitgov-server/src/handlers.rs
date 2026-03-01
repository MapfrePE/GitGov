use crate::auth::{require_admin, AuthUser};
use crate::db::{
    Database, DbError, Job, JobMetrics, NoncomplianceSignalsQuery, UpsertOrgUserInput,
};
use crate::models::*;
use crate::notifications;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use hmac::Mac;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use subtle::ConstantTimeEq;
use uuid::Uuid;

fn sanitize_db_error(e: &DbError) -> String {
    match e {
        DbError::Duplicate(_) => "Resource already exists".to_string(),
        DbError::NotFound(_) => "Resource not found".to_string(),
        DbError::DatabaseError(_) => "Internal database error".to_string(),
        DbError::SerializationError(_) => "Data serialization error".to_string(),
    }
}

fn is_likely_synthetic_login(login: &str) -> bool {
    static SYNTHETIC_LOGIN_RE: OnceLock<Regex> = OnceLock::new();
    let re = SYNTHETIC_LOGIN_RE.get_or_init(|| {
        Regex::new(r"^(alias_|erase_ok_|hb_user_|user_[0-9a-f]{6,}|test_?user|golden_?test|smoke|manual-check|victim_|dev_team_|e2e_)")
            .expect("valid synthetic login regex")
    });
    re.is_match(login)
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub github_webhook_secret: Option<String>,
    pub github_personal_access_token: Option<String>,
    pub jenkins_webhook_secret: Option<String>,
    pub jira_webhook_secret: Option<String>,
    pub start_time: Instant,
    pub worker_id: String,
    pub http_client: reqwest::Client,
    pub alert_webhook_url: Option<String>,
    pub strict_actor_match: bool,
    pub reject_synthetic_logins: bool,
    /// Gemini API key for conversational chat (env: GEMINI_API_KEY)
    pub llm_api_key: Option<String>,
    /// Gemini model for conversational chat (env: GEMINI_MODEL)
    pub llm_model: String,
    /// Webhook URL to notify on new feature requests (env: FEATURE_REQUEST_WEBHOOK_URL)
    pub feature_request_webhook_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub database: String,
}

pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: "supabase".to_string(),
    })
}

// ============================================================================
// DETAILED HEALTH CHECK (for desktop connection status)
// ============================================================================

pub async fn detailed_health(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let start = Instant::now();
    
    let (db_connected, latency_ms, pending_events) = match state.db.health_check().await {
        Ok((connected, count)) => {
            let latency = start.elapsed().as_millis() as i64;
            (connected, Some(latency), Some(count))
        }
        Err(_) => (false, None, None),
    };

    Json(DetailedHealthResponse {
        status: if db_connected { "ok" } else { "degraded" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: DatabaseHealth {
            connected: db_connected,
            latency_ms,
            pending_events,
        },
        uptime_seconds: state.start_time.elapsed().as_secs() as i64,
        timestamp: chrono::Utc::now().timestamp_millis(),
    })
}

// ============================================================================
// JENKINS INTEGRATION (V1.2-A)
// ============================================================================

pub async fn ingest_jenkins_pipeline_event(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<JenkinsPipelineEventInput>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    if let Some(expected_secret) = state.jenkins_webhook_secret.as_deref() {
        let provided_secret = headers
            .get("x-gitgov-jenkins-secret")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .unwrap_or_default();

        if provided_secret.is_empty() || provided_secret.as_bytes().ct_eq(expected_secret.as_bytes()).unwrap_u8() != 1 {
            tracing::warn!("Rejected Jenkins pipeline event due to missing/invalid secret header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(JenkinsPipelineEventResponse {
                    accepted: false,
                    duplicate: false,
                    pipeline_event_id: None,
                    error: Some("Invalid Jenkins webhook secret".to_string()),
                }),
            );
        }
    }

    if payload.pipeline_id.trim().is_empty() || payload.job_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some("pipeline_id and job_name are required".to_string()),
            }),
        );
    }

    let Some(status) = PipelineStatus::from_str(payload.status.trim()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some("Invalid status. Use: success, failure, aborted, unstable".to_string()),
            }),
        );
    };

    let org_id = if let Some(repo_full_name) = payload.repo_full_name.as_deref() {
        match state.db.get_repo_by_full_name(repo_full_name).await {
            Ok(Some(repo)) => repo.org_id,
            Ok(None) => {
                let guessed_org = repo_full_name.split('/').next().unwrap_or_default();
                if guessed_org.is_empty() {
                    None
                } else {
                    state.db.get_org_by_login(guessed_org).await.ok().flatten().map(|o| o.id)
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let raw_payload = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
    let event = PipelineEvent {
        id: Uuid::new_v4().to_string(),
        org_id,
        pipeline_id: payload.pipeline_id,
        job_name: payload.job_name,
        status,
        commit_sha: payload.commit_sha.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        branch: payload.branch.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        repo_full_name: payload.repo_full_name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        duration_ms: payload.duration_ms,
        triggered_by: payload.triggered_by.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        stages: payload.stages,
        artifacts: payload.artifacts,
        payload: raw_payload,
        ingested_at: payload
            .timestamp
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
    };

    tracing::info!(
        pipeline_id = %event.pipeline_id,
        job_name = %event.job_name,
        status = %event.status.as_str(),
        commit_sha = ?event.commit_sha,
        repo = ?event.repo_full_name,
        "Received Jenkins pipeline event"
    );

    match state.db.insert_pipeline_event(&event).await {
        Ok(pipeline_event_id) => (
            StatusCode::OK,
            Json(JenkinsPipelineEventResponse {
                accepted: true,
                duplicate: false,
                pipeline_event_id: Some(pipeline_event_id),
                error: None,
            }),
        ),
        Err(DbError::Duplicate(_)) => (
            StatusCode::OK,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: true,
                pipeline_event_id: None,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some(sanitize_db_error(&e)),
            }),
        ),
    }
}

// ============================================================================
// JIRA INTEGRATION (V1.2-B groundwork)
// ============================================================================

fn jira_issue_text(value: Option<&serde_json::Value>) -> Option<String> {
    value?.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn jira_issue_timestamp_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let raw = value?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn build_project_ticket_from_jira_payload(
    org_id: Option<String>,
    payload: &JiraWebhookEvent,
) -> Result<ProjectTicket, String> {
    let issue = payload.issue.as_ref().ok_or_else(|| "Missing issue object".to_string())?;
    let key = issue
        .get("key")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing issue.key".to_string())?
        .to_ascii_uppercase();

    let fields = issue.get("fields");
    let title = jira_issue_text(fields.and_then(|f| f.get("summary")));
    let status = jira_issue_text(fields.and_then(|f| f.get("status")).and_then(|s| s.get("name")));
    let assignee = jira_issue_text(fields.and_then(|f| f.get("assignee")).and_then(|a| a.get("displayName")))
        .or_else(|| jira_issue_text(fields.and_then(|f| f.get("assignee")).and_then(|a| a.get("name"))));
    let reporter = jira_issue_text(fields.and_then(|f| f.get("reporter")).and_then(|a| a.get("displayName")))
        .or_else(|| jira_issue_text(fields.and_then(|f| f.get("reporter")).and_then(|a| a.get("name"))));
    let priority = jira_issue_text(fields.and_then(|f| f.get("priority")).and_then(|p| p.get("name")));
    let ticket_type = jira_issue_text(fields.and_then(|f| f.get("issuetype")).and_then(|t| t.get("name")));
    let created_at = jira_issue_timestamp_ms(fields.and_then(|f| f.get("created")));
    let updated_at = jira_issue_timestamp_ms(fields.and_then(|f| f.get("updated")));

    let self_url = issue
        .get("self")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut text_candidates: Vec<&str> = Vec::new();
    if let Some(summary) = title.as_deref() {
        text_candidates.push(summary);
    }
    if let Some(description) = fields
        .and_then(|f| f.get("description"))
        .and_then(|d| d.as_str())
    {
        text_candidates.push(description);
    }
    let related_branches = extract_ticket_ids(&text_candidates);

    Ok(ProjectTicket {
        id: Uuid::new_v4().to_string(),
        org_id,
        ticket_id: key,
        ticket_url: self_url,
        title,
        status,
        assignee,
        reporter,
        priority,
        ticket_type,
        related_commits: vec![],
        related_prs: vec![],
        related_branches,
        created_at,
        updated_at,
        ingested_at: chrono::Utc::now().timestamp_millis(),
    })
}

pub async fn ingest_jira_webhook(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<JiraWebhookEvent>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraWebhookIngestResponse {
                accepted: false,
                duplicate: false,
                ticket_id: None,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    if let Some(expected_secret) = state.jira_webhook_secret.as_deref() {
        let provided_secret = headers
            .get("x-gitgov-jira-secret")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .unwrap_or_default();
        if provided_secret.is_empty() || provided_secret.as_bytes().ct_eq(expected_secret.as_bytes()).unwrap_u8() != 1 {
            return (
                StatusCode::UNAUTHORIZED,
                Json(JiraWebhookIngestResponse {
                    accepted: false,
                    duplicate: false,
                    ticket_id: None,
                    error: Some("Invalid Jira secret".to_string()),
                }),
            );
        }
    }

    let org_id = None;
    let ticket = match build_project_ticket_from_jira_payload(org_id, &payload) {
        Ok(ticket) => ticket,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(JiraWebhookIngestResponse {
                    accepted: false,
                    duplicate: false,
                    ticket_id: None,
                    error: Some(error),
                }),
            )
        }
    };

    let ticket_id = ticket.ticket_id.clone();
    match state.db.upsert_project_ticket(&ticket).await {
        Ok(()) => (
            StatusCode::OK,
            Json(JiraWebhookIngestResponse {
                accepted: true,
                duplicate: false,
                ticket_id: Some(ticket_id),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JiraWebhookIngestResponse {
                accepted: false,
                duplicate: false,
                ticket_id: Some(ticket_id),
                error: Some(sanitize_db_error(&e)),
            }),
        ),
    }
}

pub async fn get_jira_integration_status(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraIntegrationStatusResponse::default()),
        );
    }

    match state.db.get_jira_integration_status().await {
        Ok(status) => (StatusCode::OK, Json(status)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JiraIntegrationStatusResponse::default()),
        ),
    }
}

pub async fn get_jira_ticket_detail(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ticket_id): Path<String>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraTicketDetailResponse::default()),
        );
    }

    let normalized = ticket_id.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(JiraTicketDetailResponse::default()),
        );
    }

    match state.db.get_project_ticket_by_ticket_id(&normalized).await {
        Ok(Some(ticket)) => (
            StatusCode::OK,
            Json(JiraTicketDetailResponse { found: true, ticket: Some(ticket) }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(JiraTicketDetailResponse { found: false, ticket: None }),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JiraTicketDetailResponse::default()),
        ),
    }
}

pub async fn get_jenkins_integration_status(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JenkinsIntegrationStatusResponse {
                ok: false,
                ..Default::default()
            }),
        );
    }

    match state.db.get_jenkins_integration_status().await {
        Ok(status) => (StatusCode::OK, Json(status)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JenkinsIntegrationStatusResponse {
                ok: false,
                ..Default::default()
            }),
        ),
    }
}

fn read_metadata_commit_message(metadata: &serde_json::Value) -> Option<&str> {
    metadata
        .as_object()
        .and_then(|m| m.get("commit_message"))
        .and_then(|v| v.as_str())
}

pub async fn correlate_jira_tickets(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<JiraCorrelateRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraCorrelateResponse::default()),
        );
    }

    let hours = payload.hours.unwrap_or(24).clamp(1, 24 * 30);
    let limit = payload.limit.unwrap_or(500).clamp(1, 5000);

    let commits = match state
        .db
        .get_recent_commit_events_for_ticket_correlation(
            payload.org_name.as_deref(),
            payload.repo_full_name.as_deref(),
            hours,
            limit,
        )
        .await
    {
        Ok(commits) => commits,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(JiraCorrelateResponse::default()),
            )
        }
    };

    let mut created = 0i64;
    let mut correlated_tickets: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (commit_sha, branch, org_id, metadata, _repo_name) in &commits {
        let mut commit_sources: Vec<(&str, Vec<String>)> = Vec::new();
        if let Some(msg) = read_metadata_commit_message(metadata) {
            let tickets = extract_ticket_ids(&[msg]);
            if !tickets.is_empty() {
                commit_sources.push(("commit_message", tickets));
            }
        }
        if let Some(branch_name) = branch.as_deref() {
            let tickets = extract_ticket_ids(&[branch_name]);
            if !tickets.is_empty() {
                commit_sources.push(("branch_name", tickets));
            }
        }

        for (source, tickets) in commit_sources {
            for ticket_id in tickets {
                let correlation = CommitTicketCorrelation {
                    id: Uuid::new_v4().to_string(),
                    org_id: org_id.clone(),
                    commit_sha: commit_sha.clone(),
                    ticket_id: ticket_id.clone(),
                    correlation_source: source.to_string(),
                    confidence: if source == "commit_message" { 1.0 } else { 0.8 },
                    created_at: chrono::Utc::now().timestamp_millis(),
                };
                match state.db.insert_commit_ticket_correlation(&correlation).await {
                    Ok(true) => {
                        created += 1;
                        correlated_tickets.insert(ticket_id);
                        if let Err(e) = state
                            .db
                            .append_project_ticket_relations(
                                &correlation.ticket_id,
                                Some(&correlation.commit_sha),
                                branch.as_deref(),
                            )
                            .await
                        {
                            tracing::warn!(
                                ticket_id = %correlation.ticket_id,
                                commit_sha = %correlation.commit_sha,
                                error = %e,
                                "Failed to append Jira ticket relations after correlation"
                            );
                        }
                    }
                    Ok(false) => {}
                    Err(_) => {}
                }
            }
        }
    }

    let mut correlated_tickets: Vec<String> = correlated_tickets.into_iter().collect();
    correlated_tickets.sort();

    (
        StatusCode::OK,
        Json(JiraCorrelateResponse {
            scanned_commits: commits.len() as i64,
            correlations_created: created,
            correlated_tickets,
        }),
    )
}

pub async fn get_jira_ticket_coverage(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TicketCoverageQuery>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(TicketCoverageResponse::default()),
        );
    }

    let hours = query.hours.unwrap_or(24).clamp(1, 24 * 30);
    match state
        .db
        .get_ticket_coverage(
            query.org_name.as_deref(),
            query.repo_full_name.as_deref(),
            query.branch.as_deref(),
            hours,
        )
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err(e) => {
            tracing::error!(error = %e, "Failed to compute Jira ticket coverage");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketCoverageResponse::default()),
            )
        }
    }
}

pub async fn get_jenkins_commit_correlations(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<JenkinsCorrelationFilter>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JenkinsCorrelationsResponse::default()),
        );
    }

    let filter = JenkinsCorrelationFilter {
        limit: if filter.limit == 0 { 20 } else { filter.limit },
        ..filter
    };

    match state.db.get_commit_pipeline_correlations(&filter).await {
        Ok(correlations) => (
            StatusCode::OK,
            Json(JenkinsCorrelationsResponse { correlations }),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JenkinsCorrelationsResponse::default()),
        ),
    }
}

// ============================================================================
// COMPLIANCE DASHBOARD
// ============================================================================

pub async fn get_compliance_dashboard(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_name): Path<String>,
) -> impl IntoResponse {
    if let Err(_e) = require_admin(&auth_user) {
        return (StatusCode::FORBIDDEN, Json(ComplianceDashboard::default()));
    }

    let org = match state.db.get_org_by_login(&org_name).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ComplianceDashboard::default()),
            );
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ComplianceDashboard::default()),
            );
        }
    };

    match state.db.get_compliance_dashboard(&org.id).await {
        Ok(dashboard) => (StatusCode::OK, Json(dashboard)),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ComplianceDashboard::default())),
    }
}

// ============================================================================
// NONCOMPLIANCE SIGNALS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalsResponse {
    pub signals: Vec<NoncomplianceSignal>,
    pub total: i64,
}

pub async fn get_signals(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<SignalFilter>,
) -> impl IntoResponse {
    let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
    let offset = filter.offset as i64;

    // Non-admins can only see signals related to them
    let filter_user = if auth_user.role != UserRole::Admin {
        Some(auth_user.client_id.clone())
    } else {
        filter.user_login.clone()
    };

    let scoped_org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        filter.org_name.as_deref(),
        false,
    )
    .await
    {
        Ok(org_id) => org_id,
        Err(err) => {
            return (
                org_scope_status(err),
                Json(SignalsResponse {
                    signals: vec![],
                    total: 0,
                }),
            );
        }
    };

    let signals_query = NoncomplianceSignalsQuery {
        org_id: scoped_org_id.as_deref(),
        confidence: filter.confidence.as_deref(),
        status: filter.status.as_deref(),
        signal_type: filter.signal_type.as_deref(),
        actor_login: filter_user.as_deref(),
        limit,
        offset,
    };

    match state.db.get_noncompliance_signals(&signals_query).await {
        Ok((signals, total)) => (StatusCode::OK, Json(SignalsResponse { signals, total })),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(SignalsResponse { signals: vec![], total: 0 })),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalFilter {
    pub org_name: Option<String>,
    pub confidence: Option<String>,
    pub status: Option<String>,
    pub signal_type: Option<String>,
    pub user_login: Option<String>,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSignalRequest {
    pub status: String,
    pub notes: Option<String>,
}

pub async fn update_signal(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(signal_id): Path<String>,
    Json(payload): Json<UpdateSignalRequest>,
) -> impl IntoResponse {
    let signal = match state.db.get_signal_by_id(&signal_id).await {
        Ok(Some(signal)) => signal,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Signal not found"})),
            );
        }
        Err(e) => {
            tracing::error!("Failed to load signal {}: {}", signal_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal database error"})),
            );
        }
    };

    if auth_user.role != UserRole::Admin {
        let same_user = signal.actor_login == auth_user.client_id;
        let same_org = auth_user.org_id.is_some() && auth_user.org_id == signal.org_id;
        if !same_user && !same_org {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "Insufficient permissions to update this signal"})),
            );
        }
    }

    match state.db.update_signal_status(
        &signal_id,
        &payload.status,
        &auth_user.client_id,
        payload.notes.as_deref(),
    ).await {
        Ok(()) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(e) => {
            tracing::error!("Failed to update signal: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": sanitize_db_error(&e)})))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfirmSignalRequest {
    pub severity: String,
}

pub async fn confirm_signal(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(signal_id): Path<String>,
    Json(payload): Json<ConfirmSignalRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    match state.db.confirm_signal_as_violation(
        &signal_id,
        &auth_user.client_id,
        &payload.severity,
    ).await {
        Ok(violation_id) => {
            tracing::info!("Signal {} confirmed as violation {} by {}", signal_id, violation_id, auth_user.client_id);
            // Admin audit log — append-only, non-fatal
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "confirm_signal".to_string(),
                target_type: Some("signal".to_string()),
                target_id: Some(signal_id.clone()),
                metadata: serde_json::json!({ "violation_id": violation_id, "severity": payload.severity }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (confirm_signal): {}", e);
            }
            // Fire-and-forget alert
            if let Some(ref webhook_url) = state.alert_webhook_url {
                let text = notifications::format_signal_confirmed_alert(
                    &payload.severity,
                    &auth_user.client_id,
                    None,
                );
                let client = state.http_client.clone();
                let url = webhook_url.clone();
                tokio::spawn(async move {
                    notifications::send_alert(&client, &url, text).await;
                });
            }
            (StatusCode::OK, Json(json!({
                "success": true,
                "violation_id": violation_id
            })))
        }
        Err(e) => {
            tracing::error!("Failed to confirm signal: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": sanitize_db_error(&e)})))
        }
    }
}

pub async fn trigger_detection(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_name): Path<String>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    let org = match state.db.get_org_by_login(&org_name).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Organization not found"})),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get org: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal database error"})),
            );
        }
    };

    match state.db.detect_noncompliance_signals(&org.id).await {
        Ok(count) => (StatusCode::OK, Json(json!({"signals_created": count}))),
        Err(e) => {
            tracing::error!("Failed to detect signals: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal database error"})))
        }
    }
}

// ============================================================================
// VIOLATION DECISIONS (v3 schema)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct AddDecisionRequest {
    pub decision_type: String,
    pub decided_by: String,
    pub notes: Option<String>,
    pub evidence: Option<serde_json::Value>,
}

pub async fn add_violation_decision(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(violation_id): Path<String>,
    Json(payload): Json<AddDecisionRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    let valid_types = ["acknowledged", "false_positive", "resolved", "escalated", "dismissed", "wont_fix"];
    if !valid_types.contains(&payload.decision_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid decision_type",
                "valid_types": valid_types
            })),
        );
    }

    match state.db.add_violation_decision(
        &violation_id,
        &payload.decision_type,
        &auth_user.client_id,
        payload.notes.as_deref(),
        payload.evidence.clone(),
    ).await {
        Ok(decision_id) => {
            tracing::info!(
                violation_id = %violation_id,
                decision_type = %payload.decision_type,
                decided_by = %auth_user.client_id,
                admin = %auth_user.client_id,
                "Violation decision added"
            );
            (
                StatusCode::OK,
                Json(json!({
                    "decision_id": decision_id,
                    "violation_id": violation_id,
                    "decision_type": payload.decision_type,
                    "decided_by": auth_user.client_id
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to add violation decision: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": sanitize_db_error(&e)})))
        }
    }
}

pub async fn get_violation_decisions(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(violation_id): Path<String>,
) -> impl IntoResponse {
    if auth_user.role != UserRole::Admin {
        let scope = match state.db.get_violation_scope(&violation_id).await {
            Ok(Some(scope)) => scope,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"decisions": [], "error": "Violation not found"})),
                );
            }
            Err(e) => {
                tracing::error!("Failed to load violation scope: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"decisions": [], "error": "Internal database error"})),
                );
            }
        };

        let (violation_org_id, violation_user_login) = scope;
        let same_user = violation_user_login
            .as_deref()
            .map(|login| login == auth_user.client_id)
            .unwrap_or(false);
        let same_org = auth_user.org_id.is_some() && auth_user.org_id == violation_org_id;

        if !same_user && !same_org {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"decisions": [], "error": "Insufficient permissions"})),
            );
        }
    }

    match state.db.get_violation_decisions(&violation_id).await {
        Ok(decisions) => (StatusCode::OK, Json(json!({"decisions": decisions}))),
        Err(e) => {
            tracing::error!("Failed to get violation decisions: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"decisions": [], "error": "Internal database error"})))
        }
    }
}

// ============================================================================
// POLICY HISTORY
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyHistoryResponse {
    pub history: Vec<PolicyHistory>,
}

pub async fn get_policy_history(
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
) -> impl IntoResponse {
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyHistoryResponse { history: vec![] }),
            );
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyHistoryResponse { history: vec![] }),
            );
        }
    };

    match state.db.get_policy_history(&repo.id).await {
        Ok(history) => (StatusCode::OK, Json(PolicyHistoryResponse { history })),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(PolicyHistoryResponse { history: vec![] })),
    }
}

// ============================================================================
// EXPORT ENDPOINT
// ============================================================================

pub async fn export_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExportRequest>,
) -> impl IntoResponse {
    let org_id = if let Some(ref org_name) = payload.org_name {
        state.db.get_org_by_login(org_name).await.ok().flatten().map(|o| o.id)
    } else {
        None
    };

    let mut filter = EventFilter {
        start_date: payload.start_date,
        end_date: payload.end_date,
        org_name: payload.org_name.clone(),
        ..Default::default()
    };
    if auth_user.role != UserRole::Admin {
        filter.user_login = Some(auth_user.client_id.clone());
    }

    let events = match state.db.get_events_for_export(&filter).await {
        Ok(e) => e,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExportResponse {
                    id: String::new(),
                    export_type: payload.export_type.clone(),
                    record_count: 0,
                    content_hash: String::new(),
                    data: None,
                    created_at: 0,
                }),
            );
        }
    };

    let record_count = events.len() as i32;
    let data_json = serde_json::to_string(&events).unwrap_or_default();
    let content_hash = format!("{:x}", Sha256::digest(data_json.as_bytes()));

    let export_log = ExportLog {
        id: Uuid::new_v4().to_string(),
        org_id: if auth_user.role == UserRole::Admin { org_id } else { auth_user.org_id.clone().or(org_id) },
        exported_by: auth_user.client_id.clone(),
        export_type: payload.export_type.clone(),
        date_range_start: payload.start_date,
        date_range_end: payload.end_date,
        filters: payload.filters.unwrap_or(serde_json::Value::Null),
        record_count,
        content_hash: Some(content_hash.clone()),
        file_path: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    let _ = state.db.create_export_log(&export_log).await;
    // Admin audit log — append-only, non-fatal
    let audit_entry = AdminAuditLogEntry {
        id: Uuid::new_v4().to_string(),
        actor_client_id: auth_user.client_id.clone(),
        action: "export_events".to_string(),
        target_type: Some("export_log".to_string()),
        target_id: Some(export_log.id.clone()),
        metadata: serde_json::json!({ "record_count": record_count, "export_type": export_log.export_type }),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
        tracing::warn!("Failed to write admin audit log (export_events): {}", e);
    }

    let response = ExportResponse {
        id: export_log.id,
        export_type: payload.export_type,
        record_count,
        content_hash,
        data: Some(serde_json::to_value(&events).unwrap_or(serde_json::Value::Null)),
        created_at: export_log.created_at,
    };

    (StatusCode::OK, Json(response))
}

pub async fn list_exports(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state.db.list_export_logs(auth_user.org_id.as_deref()).await {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

// ============================================================================
// GITHUB WEBHOOK HANDLER
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub received: bool,
    pub delivery_id: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_github_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let delivery_id = headers
        .get("X-GitHub-Delivery")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&Uuid::new_v4().to_string())
        .to_string();

    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let signature = headers
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Validate HMAC signature if secret is configured
    if let Some(ref secret) = state.github_webhook_secret {
        if let Some(ref sig) = signature {
            if !validate_github_signature(secret, &body, sig) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(WebhookResponse {
                        received: false,
                        delivery_id: delivery_id.clone(),
                        event_type: event_type.clone(),
                        processed: Some(false),
                        error: Some("Invalid signature".to_string()),
                    }),
                );
            }
        } else {
            return (
                StatusCode::UNAUTHORIZED,
                Json(WebhookResponse {
                    received: false,
                    delivery_id: delivery_id.clone(),
                    event_type: event_type.clone(),
                    processed: Some(false),
                    error: Some("Missing signature".to_string()),
                }),
            );
        }
    }

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::warn!("Invalid JSON webhook payload: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse {
                    received: false,
                    delivery_id: delivery_id.clone(),
                    event_type: event_type.clone(),
                    processed: Some(false),
                    error: Some("Invalid JSON payload".to_string()),
                }),
            );
        }
    };

    // Store raw webhook event for debugging
    let webhook_id = match state.db.store_webhook_event(
        &delivery_id,
        &event_type,
        signature.as_deref(),
        &payload,
    ).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!("Failed to store webhook event: {}", e);
            None
        }
    };

    // Process the webhook based on event type
    let process_result = match event_type.as_str() {
        "push" => process_push_event(&state, &delivery_id, &payload).await,
        "create" => process_create_event(&state, &delivery_id, &payload).await,
        "pull_request" => process_pull_request_event(&state, &delivery_id, &payload).await,
        _ => {
            tracing::debug!("Unhandled event type: {}", event_type);
            Ok(())
        }
    };

    // Mark webhook as processed
    if let Some(ref id) = webhook_id {
        let error_msg = if process_result.is_err() {
            process_result.as_ref().err().map(|e| e.to_string())
        } else {
            None
        };
        let _ = state.db.mark_webhook_processed(id, error_msg.as_deref()).await;
    }

    match process_result {
        Ok(()) => (
            StatusCode::OK,
            Json(WebhookResponse {
                received: true,
                delivery_id,
                event_type,
                processed: Some(true),
                error: None,
            }),
        ),
        Err(e) if e.to_string().contains("duplicate") || e.to_string().contains("Duplicate") => {
            tracing::info!("Duplicate webhook received: delivery_id={}", delivery_id);
            (
                StatusCode::OK,
                Json(WebhookResponse {
                    received: true,
                    delivery_id,
                    event_type,
                    processed: Some(true),
                    error: Some("Duplicate delivery_id - already processed".to_string()),
                }),
            )
        }
        Err(_e) => (
            StatusCode::OK,
            Json(WebhookResponse {
                received: true,
                delivery_id,
                event_type,
                processed: Some(false),
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

fn validate_github_signature(secret: &str, payload_bytes: &[u8], signature: &str) -> bool {
    let signature_hex = match signature.strip_prefix("sha256=") {
        Some(hex) => hex,
        None => return false,
    };
    let signature_bytes = match hex::decode(signature_hex) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let mut mac = match <hmac::Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(payload_bytes);
    mac.verify_slice(&signature_bytes).is_ok()
}

async fn process_push_event(
    state: &Arc<AppState>,
    delivery_id: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let push: PushEvent = serde_json::from_value(payload.clone())
        .map_err(|e| format!("Failed to parse push event: {}", e))?;

    // Extract org/repo info
    let (org_id, repo_id) = get_or_create_org_repo(&state.db, &push.repository).await?;

    // Extract commit SHAs
    let commit_shas: Vec<String> = push.commits.iter().map(|c| c.id.clone()).collect();
    let commits_count = commit_shas.len() as i32;

    // Determine ref type
    let ref_type = if push.r#ref.starts_with("refs/tags/") {
        "tag"
    } else {
        "branch"
    };

    let ref_name = push.r#ref
        .strip_prefix("refs/heads/")
        .or_else(|| push.r#ref.strip_prefix("refs/tags/"))
        .unwrap_or(&push.r#ref)
        .to_string();

    let actor_login = push.sender.login.clone();
    // Keep canonical type as "push" for compatibility with existing stats/signals SQL.
    let event_type = "push";

    if push.forced {
        tracing::warn!(
            actor = %actor_login,
            ref_name = %ref_name,
            repo = %push.repository.full_name,
            "Force push detected — history rewrite on branch"
        );
    }

    let event = GitHubEvent {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        event_type: event_type.to_string(),
        actor_login: Some(push.sender.login),
        actor_id: Some(push.sender.id),
        ref_name: Some(ref_name.clone()),
        ref_type: Some(ref_type.to_string()),
        before_sha: Some(push.before),
        after_sha: Some(push.after),
        commit_shas,
        commits_count,
        payload: payload.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    state.db.insert_github_event(&event).await
        .map_err(|e| {
            tracing::error!("Failed to insert github event: {}", e);
            "Internal database error".to_string()
        })?;

    tracing::info!(
        "Processed {} event: {} commits to {} by {}",
        event_type,
        event.commits_count,
        ref_name,
        actor_login
    );

    // Enqueue detection job instead of spawning directly (backpressure control)
    if let Some(ref org_id) = event.org_id {
        if let Err(e) = state.db.enqueue_job(org_id, "detect_signals", None).await {
            tracing::warn!("Failed to enqueue detection job for org {}: {}", org_id, e);
        }
    }

    Ok(())
}

async fn process_create_event(
    state: &Arc<AppState>,
    delivery_id: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let create: CreateEvent = serde_json::from_value(payload.clone())
        .map_err(|e| format!("Failed to parse create event: {}", e))?;

    // Extract org/repo info
    let (org_id, repo_id) = get_or_create_org_repo(&state.db, &create.repository).await?;

    let ref_name = create.r#ref.clone();
    let ref_type = create.ref_type.clone();
    let actor_login = create.sender.login.clone();

    let event = GitHubEvent {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        event_type: "create".to_string(),
        actor_login: Some(create.sender.login),
        actor_id: Some(create.sender.id),
        ref_name: Some(create.r#ref),
        ref_type: Some(create.ref_type),
        before_sha: None,
        after_sha: None,
        commit_shas: vec![],
        commits_count: 0,
        payload: payload.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    state.db.insert_github_event(&event).await
        .map_err(|e| format!("Failed to insert github event: {}", e))?;

    tracing::info!(
        "Processed create event: {} {} by {}",
        ref_type,
        ref_name,
        actor_login
    );

    Ok(())
}

#[derive(Debug, Deserialize)]
struct GitHubPrReviewUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPrReview {
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    user: Option<GitHubPrReviewUser>,
}

fn extract_final_approvers(reviews: &[GitHubPrReview]) -> Vec<String> {
    // GitHub reviews are evaluated per reviewer by latest review state.
    let mut latest_state_by_user: HashMap<String, String> = HashMap::new();

    for review in reviews {
        let Some(user) = review.user.as_ref() else { continue };
        let state = review
            .state
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_ascii_uppercase();
        if state.is_empty() {
            continue;
        }
        latest_state_by_user.insert(user.login.clone(), state);
    }

    let mut approvers: Vec<String> = latest_state_by_user
        .into_iter()
        .filter_map(|(login, state)| (state == "APPROVED").then_some(login))
        .collect();

    approvers.sort();
    approvers
}

async fn fetch_pr_approvers(
    http_client: &reqwest::Client,
    github_token: &str,
    repo_full_name: &str,
    pr_number: i32,
) -> Result<Vec<String>, String> {
    let mut all_reviews = Vec::new();
    let mut page = 1u8;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/pulls/{}/reviews?per_page=100&page={}",
            repo_full_name, pr_number, page
        );

        let response = http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", github_token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "gitgov-server")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("GitHub reviews request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("GitHub reviews API returned {}", status));
        }

        let reviews: Vec<GitHubPrReview> = response
            .json()
            .await
            .map_err(|e| format!("GitHub reviews decode failed: {}", e))?;

        let chunk_len = reviews.len();
        all_reviews.extend(reviews);

        if chunk_len < 100 || page >= 10 {
            break;
        }

        page += 1;
    }

    Ok(extract_final_approvers(&all_reviews))
}

// Processes pull_request webhook events.
// Only stores merged PRs (action == "closed" && pull_request.merged == true).
// All other actions (opened, reviewed, etc.) are silently skipped — no error.
async fn process_pull_request_event(
    state: &Arc<AppState>,
    delivery_id: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let pr = match payload.get("pull_request") {
        Some(pr) => pr,
        None => {
            tracing::debug!("pull_request event missing 'pull_request' field, delivery_id={}", delivery_id);
            return Ok(());
        }
    };

    // Only capture merged PRs
    let merged = pr.get("merged").and_then(|v| v.as_bool()).unwrap_or(false);
    if action != "closed" || !merged {
        tracing::debug!("Skipping non-merged pull_request event: action={}, delivery_id={}", action, delivery_id);
        return Ok(());
    }

    // Extract repository info for org/repo lookup
    let repo_val = match payload.get("repository") {
        Some(r) => r,
        None => {
            tracing::warn!("pull_request event missing 'repository' field, delivery_id={}", delivery_id);
            return Ok(());
        }
    };
    let repo: GitHubRepository = match serde_json::from_value(repo_val.clone()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse repository in pull_request event: {}, delivery_id={}", e, delivery_id);
            return Ok(());
        }
    };

    let (org_id, repo_id) = get_or_create_org_repo(&state.db, &repo).await?;

    let pr_number = pr.get("number").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let pr_title = pr.get("title").and_then(|v| v.as_str()).map(String::from);
    let author_login = pr.get("user").and_then(|u| u.get("login")).and_then(|v| v.as_str()).map(String::from);
    let merged_by_login = pr.get("merged_by").and_then(|u| u.get("login")).and_then(|v| v.as_str()).map(String::from);
    let head_sha = pr.get("head").and_then(|h| h.get("sha")).and_then(|v| v.as_str()).map(String::from);
    let base_branch = pr.get("base").and_then(|b| b.get("ref")).and_then(|v| v.as_str()).map(String::from);
    let approvers = match state.github_personal_access_token.as_deref() {
        Some(token) => match fetch_pr_approvers(&state.http_client, token, &repo.full_name, pr_number).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    delivery_id = %delivery_id,
                    repo = %repo.full_name,
                    pr_number,
                    error = %e,
                    "Failed to fetch PR approvers from GitHub API"
                );
                vec![]
            }
        },
        None => {
            tracing::debug!(
                delivery_id = %delivery_id,
                repo = %repo.full_name,
                pr_number,
                "GITHUB_PERSONAL_ACCESS_TOKEN not configured; storing PR merge without approvers"
            );
            vec![]
        }
    };
    let approvals_count = approvers.len() as i32;

    let mut enriched_payload = payload.clone();
    if let Some(obj) = enriched_payload.as_object_mut() {
        obj.insert(
            "gitgov".to_string(),
            serde_json::json!({
                "approvers": approvers,
                "approvals_count": approvals_count
            }),
        );
    }

    let record = PrMergeRecord {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        pr_number,
        pr_title: pr_title.clone(),
        author_login: author_login.clone(),
        merged_by_login: merged_by_login.clone(),
        head_sha,
        base_branch,
        payload: enriched_payload,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    match state.db.insert_pr_merge(&record).await {
        Ok(()) => {
            tracing::info!(
                "Processed PR merge: #{} '{}' by {} merged by {} (approvals={}), delivery_id={}",
                pr_number,
                pr_title.as_deref().unwrap_or(""),
                author_login.as_deref().unwrap_or("unknown"),
                merged_by_login.as_deref().unwrap_or("unknown"),
                approvals_count,
                delivery_id,
            );
            Ok(())
        }
        Err(DbError::Duplicate(_)) => {
            tracing::debug!("Duplicate PR merge event ignored: delivery_id={}", delivery_id);
            Ok(())
        }
        Err(e) => Err(format!("Failed to insert PR merge: {}", e)),
    }
}

async fn get_or_create_org_repo(db: &Database, repo: &GitHubRepository) -> Result<(String, String), String> {
    // Get or create org
    let org_id = if let Some(ref org) = repo.organization {
        db.upsert_org(org.id, &org.login, None, None).await
            .map_err(|e| e.to_string())?
    } else {
        // If no organization, use the owner as org
        db.upsert_org(repo.owner.id, &repo.owner.login, None, None).await
            .map_err(|e| e.to_string())?
    };

    // Get or create repo
    let repo_id = db.upsert_repo(
        Some(&org_id),
        repo.id,
        &repo.full_name,
        &repo.name,
        repo.private,
    ).await.map_err(|e| e.to_string())?;

    Ok((org_id, repo_id))
}

// ============================================================================
// CLIENT EVENTS (Batch Ingest)
// ============================================================================

pub async fn ingest_client_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(batch): Json<ClientEventBatch>,
) -> impl IntoResponse {
    let mut events = Vec::new();
    let strict_actor_match = state.strict_actor_match;

    for input in batch.events {
        if strict_actor_match
            && auth_user.role != UserRole::Admin
            && input.user_login != auth_user.client_id
        {
            tracing::warn!(
                auth_user = %auth_user.client_id,
                requested_user_login = %input.user_login,
                event_uuid = %input.event_uuid,
                "Rejecting client event due to strict actor match enforcement"
            );
            return (
                StatusCode::FORBIDDEN,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors: vec![EventError {
                        event_uuid: input.event_uuid,
                        error: "user_login must match authenticated client_id (STRICT_ACTOR_MATCH)".to_string(),
                    }],
                }),
            );
        }

        let effective_user_login = if auth_user.role == UserRole::Admin {
            input.user_login.clone()
        } else {
            auth_user.client_id.clone()
        };

        if state.reject_synthetic_logins && is_likely_synthetic_login(&effective_user_login) {
            tracing::warn!(
                auth_user = %auth_user.client_id,
                rejected_user_login = %effective_user_login,
                event_uuid = %input.event_uuid,
                "Rejecting client event due to synthetic login policy"
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors: vec![EventError {
                        event_uuid: input.event_uuid,
                        error: "synthetic user_login is not allowed in this environment".to_string(),
                    }],
                }),
            );
        }

        // Get org and repo IDs
        let requested_org_id = if let Some(ref org_name) = input.org_name {
            state.db.get_org_by_login(org_name).await
                .ok()
                .flatten()
                .map(|o| o.id)
        } else {
            None
        };

        if auth_user.role != UserRole::Admin {
            if let (Some(scoped_org_id), Some(requested_org_id)) =
                (auth_user.org_id.as_deref(), requested_org_id.as_deref())
            {
                if scoped_org_id != requested_org_id {
                    tracing::warn!(
                        auth_user = %auth_user.client_id,
                        requested_org_id = %requested_org_id,
                        scoped_org_id = %scoped_org_id,
                        event_uuid = %input.event_uuid,
                        "Rejecting client event with org mismatch"
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ClientEventResponse {
                            accepted: vec![],
                            duplicates: vec![],
                            errors: vec![EventError {
                                event_uuid: input.event_uuid,
                                error: "Event org_name is outside API key scope".to_string(),
                            }],
                        }),
                    );
                }
            }
        }

        let org_id = if auth_user.role == UserRole::Admin {
            requested_org_id
        } else {
            auth_user.org_id.clone().or(requested_org_id)
        };

        let inferred_repo_full_name = input
            .repo_full_name
            .clone()
            .or_else(|| {
                input
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("repo_name"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
            });

        let repo = if let Some(ref repo_full_name) = inferred_repo_full_name {
            state
                .db
                .get_repo_by_full_name(repo_full_name)
                .await
                .unwrap_or_default()
        } else {
            None
        };
        if auth_user.role != UserRole::Admin {
            if let (Some(scoped_org_id), Some(repo)) = (auth_user.org_id.as_deref(), repo.as_ref()) {
                if repo.org_id.as_deref() != Some(scoped_org_id) {
                    tracing::warn!(
                        auth_user = %auth_user.client_id,
                        repo = %repo.full_name,
                        event_uuid = %input.event_uuid,
                        "Rejecting client event with repo outside API key scope"
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ClientEventResponse {
                            accepted: vec![],
                            duplicates: vec![],
                            errors: vec![EventError {
                                event_uuid: input.event_uuid,
                                error: "Event repo_full_name is outside API key scope".to_string(),
                            }],
                        }),
                    );
                }
            }
        }
        let repo_id = if let Some(repo) = repo {
            Some(repo.id)
        } else if let (Some(full_name), Some(effective_org_id)) =
            (inferred_repo_full_name.as_deref(), org_id.as_deref())
        {
            let repo_name = full_name
                .split('/')
                .next_back()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(full_name);
            match state
                .db
                .upsert_repo_by_full_name(Some(effective_org_id), full_name, repo_name, true)
                .await
            {
                Ok(id) => Some(id),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        full_name = %full_name,
                        org_id = %effective_org_id,
                        event_uuid = %input.event_uuid,
                        "Failed to upsert repo from client event (non-fatal)"
                    );
                    None
                }
            }
        } else {
            None
        };

        let event = ClientEvent {
            id: Uuid::new_v4().to_string(),
            org_id,
            repo_id,
            event_uuid: input.event_uuid,
            event_type: ClientEventType::from_str(&input.event_type),
            user_login: effective_user_login,
            user_name: input.user_name,
            branch: input.branch,
            commit_sha: input.commit_sha,
            files: input.files,
            status: EventStatus::from_str(&input.status),
            reason: input.reason,
            metadata: input.metadata.unwrap_or(serde_json::Value::Null),
            client_version: batch.client_version.clone(),
            created_at: input.timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        };

        events.push(event);
    }

    match state.db.insert_client_events_batch(&events).await {
        Ok(response) => {
            // Fire-and-forget: update client_sessions last_seen + device metadata
            {
                let client_id = auth_user.client_id.clone();
                let org_id = auth_user.org_id.clone();
                // Extract device metadata from the first event that has it
                let device_meta = events
                    .iter()
                    .find_map(|e| {
                        e.metadata.get("device").cloned()
                    })
                    .unwrap_or(serde_json::json!({}));
                let db = Arc::clone(&state.db);
                tokio::spawn(async move {
                    if let Err(e) = db
                        .upsert_client_session(&client_id, org_id.as_deref(), &device_meta)
                        .await
                    {
                        tracing::debug!(error = %e, "Failed to upsert client session (non-critical)");
                    }
                });
            }

            // Fire-and-forget alert for blocked_push events
            if let Some(ref webhook_url) = state.alert_webhook_url {
                let accepted_event_ids: HashSet<&str> =
                    response.accepted.iter().map(String::as_str).collect();
                for event in &events {
                    if event.event_type == ClientEventType::BlockedPush
                        && accepted_event_ids.contains(event.event_uuid.as_str())
                    {
                        let text = notifications::format_blocked_push_alert(
                            &event.user_login,
                            event.repo_id.as_deref().unwrap_or("unknown"),
                            event.branch.as_deref().unwrap_or("unknown"),
                        );
                        let client = state.http_client.clone();
                        let url = webhook_url.clone();
                        tokio::spawn(async move {
                            notifications::send_alert(&client, &url, text).await;
                        });
                    }
                }
            }
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to insert client events batch: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors: vec![EventError {
                        event_uuid: "batch".to_string(),
                        error: "Internal database error".to_string(),
                    }],
                }),
            )
        }
    }
}

// ============================================================================
// QUERY ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsResponse {
    pub events: Vec<CombinedEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn get_logs(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<EventFilter>,
) -> impl IntoResponse {
    // Non-admins can only see their own events
    let clamped_limit = if filter.limit == 0 { 100 } else { filter.limit.min(500) };
    let mut filter = if auth_user.role != UserRole::Admin {
        EventFilter {
            user_login: Some(auth_user.client_id.clone()),
            limit: clamped_limit,
            ..filter
        }
    } else {
        EventFilter {
            limit: clamped_limit,
            ..filter
        }
    };

    let scoped_org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        filter.org_name.as_deref(),
        false,
    )
    .await
    {
        Ok(org_id) => org_id,
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (
                org_scope_status(err),
                Json(LogsResponse {
                    events: vec![],
                    error: Some(error.to_string()),
                }),
            );
        }
    };
    if scoped_org_id.is_some() {
        // Prefer UUID scope to avoid extra org_name lookups in DB query path.
        filter.org_id = scoped_org_id;
        filter.org_name = None;
    }

    match state.db.get_combined_events(&filter).await {
        Ok(events) => (StatusCode::OK, Json(LogsResponse { events, error: None })),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LogsResponse {
                events: vec![],
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

pub async fn get_stats(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (StatusCode::FORBIDDEN, Json(AuditStats::default()));
    }

    let org_id = auth_user.org_id.as_deref();
    match state.db.get_stats(org_id).await {
        Ok(mut stats) => {
            stats.pipeline = state.db.get_pipeline_health_stats(org_id).await.unwrap_or_default();
            stats.client_events.desktop_pushes_today = match state.db.get_desktop_pushes_today(org_id).await {
                Ok(count) => count,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to compute desktop pushes today for /stats");
                    0
                }
            };
            (StatusCode::OK, Json(stats))
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(AuditStats::default())),
    }
}

pub async fn get_team_overview(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TeamOverviewQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let status = if let Some(raw) = query.status.as_deref() {
        match normalize_org_user_status(Some(raw)) {
            Ok(s) => Some(s),
            Err(msg) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    let days = query.days.unwrap_or(30).clamp(1, 180);
    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        query.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            )
                .into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    match state
        .db
        .get_team_overview(&org_id, status.as_deref(), days, limit, offset)
        .await
    {
        Ok((entries, total)) => (StatusCode::OK, Json(TeamOverviewResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to load team overview");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn get_team_repos(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TeamOverviewQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let days = query.days.unwrap_or(30).clamp(1, 180);
    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        query.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            )
                .into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    match state.db.get_team_repos(&org_id, days, limit, offset).await {
        Ok((entries, total)) => (StatusCode::OK, Json(TeamReposResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to load team repo overview");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn get_daily_activity(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<DailyActivityQuery>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (StatusCode::FORBIDDEN, Json(Vec::<DailyActivityPoint>::new()));
    }

    let days = query.days.unwrap_or(14).clamp(1, 90) as i64;
    let org_id = auth_user.org_id.as_deref();

    match state.db.get_daily_activity(org_id, days).await {
        Ok(points) => (StatusCode::OK, Json(points)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Vec::<DailyActivityPoint>::new()),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub stats: AuditStats,
    pub recent_events: Vec<CombinedEvent>,
}

pub async fn get_dashboard(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (StatusCode::FORBIDDEN, Json(DashboardResponse {
            stats: AuditStats::default(),
            recent_events: vec![],
        }));
    }

    let org_id = auth_user.org_id.as_deref();
    let stats = state.db.get_stats(org_id).await.unwrap_or_default();

    let filter = EventFilter {
        limit: 10,
        org_id: auth_user.org_id.clone(),
        ..Default::default()
    };
    let recent = state.db.get_combined_events(&filter).await.unwrap_or_default();

    (StatusCode::OK, Json(DashboardResponse {
        stats,
        recent_events: recent,
    }))
}

fn repo_name_from_policy_check_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("git@") {
        if let Some(idx) = trimmed.find(':') {
            // git@github.com:owner/repo.git
            let candidate = &trimmed[idx + 1..];
            return candidate.trim_end_matches(".git").trim_matches('/').to_string();
        }
        if let Some(pos) = trimmed.find("github.com/") {
            let candidate = &trimmed[(pos + "github.com/".len())..];
            return candidate.trim_end_matches(".git").trim_matches('/').to_string();
        }
    }
    trimmed.trim_end_matches(".git").trim_matches('/').to_string()
}

fn branch_matches_policy(policy: &GitGovConfig, branch: &str) -> bool {
    if policy.branches.protected.iter().any(|b| b == branch) {
        return true;
    }

    for pattern in &policy.branches.patterns {
        if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
            if glob_pattern.matches(branch) {
                return true;
            }
        } else if pattern == branch {
            return true;
        }
    }

    false
}

fn ticket_id_regex() -> &'static Regex {
    static TICKET_ID_RE: OnceLock<Regex> = OnceLock::new();
    TICKET_ID_RE.get_or_init(|| {
        Regex::new(r"\b([A-Z][A-Z0-9]{1,15}-[0-9]{1,9})\b")
            .expect("valid ticket id regex")
    })
}

fn extract_ticket_ids(texts: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for text in texts {
        for captures in ticket_id_regex().captures_iter(text) {
            if let Some(ticket) = captures.get(1) {
                let normalized = ticket.as_str().to_ascii_uppercase();
                if seen.insert(normalized.clone()) {
                    found.push(normalized);
                }
            }
        }
    }

    found
}

pub async fn policy_check(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PolicyCheckRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(PolicyCheckResponse {
                advisory: true,
                allowed: false,
                reasons: vec!["Admin access required".to_string()],
                warnings: vec![],
                evaluated_rules: vec![],
            }),
        );
    }

    if payload.repo.trim().is_empty() || payload.branch.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(PolicyCheckResponse {
                advisory: true,
                allowed: false,
                reasons: vec!["repo and branch are required".to_string()],
                warnings: vec![],
                evaluated_rules: vec![],
            }),
        );
    }

    let repo_name = repo_name_from_policy_check_input(&payload.repo);
    let mut response = PolicyCheckResponse {
        advisory: true,
        allowed: true,
        reasons: vec![],
        warnings: vec![],
        evaluated_rules: vec![
            "repo_exists".to_string(),
            "policy_exists".to_string(),
            "branch_matches_policy".to_string(),
        ],
    };

    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            response.allowed = false;
            response.reasons.push("Repository not found in GitGov".to_string());
            return (StatusCode::OK, Json(response));
        }
        Err(_) => {
            response.allowed = false;
            response.reasons.push("Internal database error".to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let policy = match state.db.get_policy(&repo.id).await {
        Ok(Some(policy)) => policy,
        Ok(None) => {
            response.allowed = false;
            response.reasons.push("No policy configured for repository".to_string());
            return (StatusCode::OK, Json(response));
        }
        Err(_) => {
            response.allowed = false;
            response.reasons.push("Internal database error".to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let branch = payload.branch.trim();
    if !branch_matches_policy(&policy.config, branch) {
        response.allowed = false;
        response
            .reasons
            .push(format!("Branch '{}' does not match configured policy patterns/protected branches", branch));
    }

    response.warnings.push(
        "Advisory mode: author/role validation not enforced yet in V1.2-A".to_string(),
    );
    response.warnings.push(
        "Advisory mode: bypass/drift checks are not fully integrated into policy/check yet".to_string(),
    );
    if payload.commit.as_deref().unwrap_or_default().is_empty() {
        response
            .warnings
            .push("Commit SHA not provided; commit-specific checks skipped".to_string());
    }

    (StatusCode::OK, Json(response))
}

// ============================================================================
// POLICIES
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyApiResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<GitGovConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn get_policy(
    Extension(_auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
) -> impl IntoResponse {
    // First get repo ID by full_name
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Repository not found".to_string()),
                }),
            );
        }
        Err(_e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    match state.db.get_policy(&repo.id).await {
        Ok(Some(policy)) => (
            StatusCode::OK,
            Json(PolicyApiResponse {
                version: Some(policy.version),
                checksum: Some(policy.checksum),
                config: Some(policy.config),
                updated_at: Some(policy.updated_at),
                error: None,
            }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Policy not found".to_string()),
            }),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

pub async fn override_policy(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
    Json(config): Json<GitGovConfig>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    // First get repo ID by full_name
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Repository not found".to_string()),
                }),
            );
        }
        Err(_e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    let config_json = match serde_json::to_string(&config) {
        Ok(json) => json,
        Err(_e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    let checksum = format!("{:x}", Sha256::digest(config_json.as_bytes()));

    // Record that this is an override
    tracing::warn!(
        "Policy override for {} by {} (is_override=true)",
        repo_name,
        auth_user.client_id
    );

    match state.db.save_policy(&repo.id, &config, &checksum, &auth_user.client_id).await {
        Ok(()) => {
            // Admin audit log — append-only, non-fatal
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "policy_override".to_string(),
                target_type: Some("repo".to_string()),
                target_id: Some(repo.id.clone()),
                metadata: serde_json::json!({
                    "repo_name": repo_name,
                    "checksum": checksum
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (policy_override): {}", e);
            }

            (
                StatusCode::OK,
                Json(PolicyApiResponse {
                    version: Some("1.0".to_string()),
                    checksum: Some(checksum),
                    config: Some(config),
                    updated_at: Some(chrono::Utc::now().timestamp_millis()),
                    error: None,
                }),
            )
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

// ============================================================================
// ORGS (admin provisioning)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrgRequest {
    /// GitHub login / slug — must be unique (e.g. "rimac")
    pub login: String,
    /// Human-readable display name (e.g. "Rimac Seguros")
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrgResponse {
    pub org_id: String,
    pub login: String,
    /// true = newly created, false = already existed (idempotent)
    pub created: bool,
}

pub async fn create_org(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateOrgRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    // Check if org already exists — upsert_org is idempotent on (login)
    let already_exists = state.db.get_org_by_login(&payload.login).await
        .unwrap_or(None)
        .is_some();

    // Manually provisioned orgs must upsert by login (not by github_id) to avoid collisions.
    match state
        .db
        .upsert_org_by_login(&payload.login, payload.name.as_deref(), None)
        .await
    {
        Ok(org_id) => (
            if already_exists { StatusCode::OK } else { StatusCode::CREATED },
            Json(CreateOrgResponse {
                org_id,
                login: payload.login,
                created: !already_exists,
            }),
        ).into_response(),
        Err(e) => {
            tracing::error!(error = %e, login = %payload.login, "Failed to upsert org");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

fn parse_user_role_strict(raw: Option<&str>) -> Result<UserRole, &'static str> {
    match raw.unwrap_or("Developer").trim() {
        "Admin" => Ok(UserRole::Admin),
        "Architect" => Ok(UserRole::Architect),
        "Developer" => Ok(UserRole::Developer),
        "PM" => Ok(UserRole::PM),
        _ => Err("role must be one of: Admin, Architect, Developer, PM"),
    }
}

fn normalize_org_user_status(raw: Option<&str>) -> Result<String, &'static str> {
    let normalized = raw.unwrap_or("active").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "active" | "disabled" => Ok(normalized),
        _ => Err("status must be 'active' or 'disabled'"),
    }
}

fn normalize_org_invitation_status_filter(raw: Option<&str>) -> Result<String, &'static str> {
    let normalized = raw.unwrap_or("").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "pending" | "accepted" | "revoked" | "expired" => Ok(normalized),
        _ => Err("status must be one of: pending, accepted, revoked, expired"),
    }
}

// ============================================================================
// ORG INVITATIONS (admin onboarding)
// ============================================================================

pub async fn create_org_invitation(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateOrgInvitationRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let invite_email = payload.invite_email.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let invite_login = payload.invite_login.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if invite_email.is_none() && invite_login.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invite_email or invite_login is required" })),
        ).into_response();
    }

    let role = match parse_user_role_strict(payload.role.as_deref()) {
        Ok(role) => role,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            ).into_response();
        }
    };

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        payload.org_name.as_deref(),
        true,
    ).await {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            ).into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    let expires_days = payload.expires_in_days.unwrap_or(7).clamp(1, 30);
    let expires_at = chrono::Utc::now() + chrono::Duration::days(expires_days);
    let invite_token = Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", Sha256::digest(invite_token.as_bytes()));

    match state.db.create_org_invitation(&crate::db::CreateOrgInvitationInput {
        org_id: &org_id,
        invite_email,
        invite_login,
        role: role.as_str(),
        token_hash: &token_hash,
        invited_by: &auth_user.client_id,
        expires_at,
    }).await {
        Ok(invitation) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "create_org_invitation".to_string(),
                target_type: Some("org_invitation".to_string()),
                target_id: Some(invitation.id.clone()),
                metadata: serde_json::json!({
                    "org_id": invitation.org_id,
                    "invite_email": invitation.invite_email,
                    "invite_login": invitation.invite_login,
                    "role": invitation.role,
                    "expires_at": invitation.expires_at
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (create_org_invitation)");
            }

            (
                StatusCode::CREATED,
                Json(CreateOrgInvitationResponse {
                    invitation,
                    invite_token,
                }),
            ).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to create org invitation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

pub async fn list_org_invitations(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<OrgInvitationsQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let status = if let Some(raw) = query.status.as_deref() {
        match normalize_org_invitation_status_filter(Some(raw)) {
            Ok(value) => Some(value),
            Err(msg) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": msg })),
                ).into_response();
            }
        }
    } else {
        None
    };

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        query.org_name.as_deref(),
        true,
    ).await {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            ).into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_org_invitations(&org_id, status.as_deref(), limit, offset)
        .await
    {
        Ok((entries, total)) => (StatusCode::OK, Json(OrgInvitationsResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to list org invitations");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

pub async fn resend_org_invitation(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(invitation_id): Path<String>,
    Json(payload): Json<ResendOrgInvitationRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let invite_token = Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", Sha256::digest(invite_token.as_bytes()));
    let expires_days = payload.expires_in_days.unwrap_or(7).clamp(1, 30);
    let expires_at = chrono::Utc::now() + chrono::Duration::days(expires_days);

    match state
        .db
        .resend_org_invitation(
            &invitation_id,
            auth_user.org_id.as_deref(),
            &token_hash,
            &auth_user.client_id,
            expires_at,
        )
        .await
    {
        Ok(Some(invitation)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "resend_org_invitation".to_string(),
                target_type: Some("org_invitation".to_string()),
                target_id: Some(invitation.id.clone()),
                metadata: serde_json::json!({
                    "org_id": invitation.org_id,
                    "invite_email": invitation.invite_email,
                    "invite_login": invitation.invite_login,
                    "role": invitation.role,
                    "expires_at": invitation.expires_at
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (resend_org_invitation)");
            }

            (
                StatusCode::OK,
                Json(CreateOrgInvitationResponse {
                    invitation,
                    invite_token,
                }),
            ).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "invitation not found or already accepted" })),
        ).into_response(),
        Err(e) => {
            tracing::error!(error = %e, invitation_id = %invitation_id, "Failed to resend org invitation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

pub async fn revoke_org_invitation(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(invitation_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state
        .db
        .revoke_org_invitation(&invitation_id, auth_user.org_id.as_deref(), &auth_user.client_id)
        .await
    {
        Ok(Some(invitation)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "revoke_org_invitation".to_string(),
                target_type: Some("org_invitation".to_string()),
                target_id: Some(invitation.id.clone()),
                metadata: serde_json::json!({
                    "org_id": invitation.org_id,
                    "invite_email": invitation.invite_email,
                    "invite_login": invitation.invite_login
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (revoke_org_invitation)");
            }
            (StatusCode::OK, Json(invitation)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "invitation not found or not pending" })),
        ).into_response(),
        Err(e) => {
            tracing::error!(error = %e, invitation_id = %invitation_id, "Failed to revoke org invitation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

pub async fn preview_org_invitation(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "token is required" })),
        ).into_response();
    }

    let token_hash = format!("{:x}", Sha256::digest(trimmed.as_bytes()));
    match state.db.get_org_invitation_by_token_hash(&token_hash).await {
        Ok(Some(invitation)) => (StatusCode::OK, Json(invitation)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "invitation not found" })),
        ).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to preview invitation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

pub async fn accept_org_invitation(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AcceptOrgInvitationRequest>,
) -> impl IntoResponse {
    let token = payload.token.trim();
    if token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "token is required" })),
        ).into_response();
    }
    let login = payload.login.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));

    match state.db.accept_org_invitation(&token_hash, login).await {
        Ok(Some(result)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: result.org_user.login.clone(),
                action: "accept_org_invitation".to_string(),
                target_type: Some("org_invitation".to_string()),
                target_id: Some(result.invitation.id.clone()),
                metadata: serde_json::json!({
                    "org_id": result.invitation.org_id,
                    "client_id": result.org_user.login,
                    "role": result.invitation.role
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (accept_org_invitation)");
            }

            (
                StatusCode::OK,
                Json(AcceptOrgInvitationResponse {
                    invitation: result.invitation,
                    client_id: result.org_user.login,
                    role: result.org_user.role,
                    org_id: result.org_user.org_id,
                    api_key: result.api_key,
                }),
            ).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "invitation not found, expired or already used" })),
        ).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to accept invitation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            ).into_response()
        }
    }
}

// ============================================================================
// ORG USERS (admin provisioning)
// ============================================================================

pub async fn create_org_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateOrgUserRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let login = payload.login.trim().to_string();
    if login.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "login cannot be empty" })),
        )
            .into_response();
    }

    let role = match parse_user_role_strict(payload.role.as_deref()) {
        Ok(r) => r,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };

    let status = match normalize_org_user_status(payload.status.as_deref()) {
        Ok(s) => s,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        payload.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            )
                .into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    match state
        .db
        .upsert_org_user(&UpsertOrgUserInput {
            org_id: &org_id,
            login: &login,
            display_name: payload.display_name.as_deref(),
            email: payload.email.as_deref(),
            role: role.as_str(),
            status: &status,
            actor: &auth_user.client_id,
        })
        .await
    {
        Ok((user, created)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: if created { "create_org_user" } else { "update_org_user" }.to_string(),
                target_type: Some("org_user".to_string()),
                target_id: Some(user.id.clone()),
                metadata: serde_json::json!({
                    "org_id": user.org_id,
                    "login": user.login,
                    "role": user.role,
                    "status": user.status,
                    "created": created
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (org_user upsert)");
            }

            (
                if created { StatusCode::CREATED } else { StatusCode::OK },
                Json(CreateOrgUserResponse { user, created }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, login = %login, org_id = %org_id, "Failed to upsert org user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn list_org_users(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<OrgUsersQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let status = if let Some(raw) = query.status.as_deref() {
        match normalize_org_user_status(Some(raw)) {
            Ok(s) => Some(s),
            Err(msg) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        query.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            )
                .into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_org_users(&org_id, status.as_deref(), limit, offset)
        .await
    {
        Ok((entries, total)) => (StatusCode::OK, Json(OrgUsersResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to list org users");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn update_org_user_status(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_user_id): Path<String>,
    Json(payload): Json<UpdateOrgUserStatusRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let status = match normalize_org_user_status(Some(payload.status.as_str())) {
        Ok(s) => s,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };

    match state
        .db
        .update_org_user_status(
            &org_user_id,
            auth_user.org_id.as_deref(),
            &status,
            &auth_user.client_id,
        )
        .await
    {
        Ok(Some(user)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "update_org_user_status".to_string(),
                target_type: Some("org_user".to_string()),
                target_id: Some(user.id.clone()),
                metadata: serde_json::json!({
                    "login": user.login,
                    "status": user.status
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (org_user status)");
            }
            (StatusCode::OK, Json(user)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "org_user not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_user_id = %org_user_id, "Failed to update org user status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn create_api_key_for_org_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_user_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let org_user = match state
        .db
        .get_org_user_by_id(&org_user_id, auth_user.org_id.as_deref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: "".to_string(),
                    error: Some("org_user not found".to_string()),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, org_user_id = %org_user_id, "Failed to load org user for API key creation");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: "".to_string(),
                    error: Some("Internal database error".to_string()),
                }),
            )
                .into_response();
        }
    };

    if org_user.status != "active" {
        return (
            StatusCode::CONFLICT,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: org_user.login,
                error: Some("org_user must be active to issue API key".to_string()),
            }),
        )
            .into_response();
    }

    let api_key = Uuid::new_v4().to_string();
    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
    let role = UserRole::from_str(&org_user.role);

    match state
        .db
        .create_api_key(
            &key_hash,
            &org_user.login,
            Some(org_user.org_id.as_str()),
            &role,
        )
        .await
    {
        Ok(()) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "create_api_key_for_org_user".to_string(),
                target_type: Some("org_user".to_string()),
                target_id: Some(org_user_id),
                metadata: serde_json::json!({
                    "client_id": org_user.login,
                    "role": org_user.role,
                    "org_id": org_user.org_id
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (create_api_key_for_org_user)");
            }

            (
                StatusCode::CREATED,
                Json(ApiKeyResponse {
                    api_key: Some(api_key),
                    client_id: org_user.login,
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, org_user_id = %org_user_id, "Failed to create API key for org user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: org_user.login,
                    error: Some("Internal database error".to_string()),
                }),
            )
                .into_response()
        }
    }
}

// ============================================================================
// API KEYS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyRequest {
    pub client_id: String,
    pub org_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn create_api_key(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ApiKeyRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: payload.client_id,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    let role = UserRole::from_str(&payload.role);

    // Resolve requested org by login (if provided in payload).
    let requested_org_id = if let Some(ref org_name) = payload.org_name {
        state.db.get_org_by_login(org_name).await
            .ok()
            .flatten()
            .map(|o| o.id)
    } else {
        None
    };

    // If admin key is org-scoped, key creation must stay inside that org.
    let effective_org_id = if let Some(scoped_org_id) = auth_user.org_id.as_deref() {
        match requested_org_id {
            Some(ref requested) if requested == scoped_org_id => Some(requested.clone()),
            Some(_) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ApiKeyResponse {
                        api_key: None,
                        client_id: payload.client_id,
                        error: Some("Requested org_name is outside API key scope".to_string()),
                    }),
                );
            }
            None => Some(scoped_org_id.to_string()),
        }
    } else {
        requested_org_id
    };

    let api_key = Uuid::new_v4().to_string();
    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));

    match state
        .db
        .create_api_key(&key_hash, &payload.client_id, effective_org_id.as_deref(), &role)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(ApiKeyResponse {
                api_key: Some(api_key),
                client_id: payload.client_id,
                error: None,
            }),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: payload.client_id,
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

pub async fn get_me(
    Extension(auth_user): Extension<AuthUser>,
) -> impl IntoResponse {
    let response = MeResponse {
        client_id: auth_user.client_id,
        role: auth_user.role.as_str().to_string(),
        org_id: auth_user.org_id,
    };
    (StatusCode::OK, Json(response))
}

pub async fn list_api_keys(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state.db.list_api_keys(auth_user.org_id.as_deref()).await {
        Ok(keys) => (StatusCode::OK, Json(keys)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

pub async fn revoke_api_key(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(key_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state
        .db
        .revoke_api_key(&key_id, auth_user.org_id.as_deref())
        .await
    {
        Ok(true) => {
            // Admin audit log — append-only, non-fatal
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "revoke_api_key".to_string(),
                target_type: Some("api_key".to_string()),
                target_id: Some(key_id.clone()),
                metadata: serde_json::Value::Null,
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (revoke_api_key): {}", e);
            }
            (
                StatusCode::OK,
                Json(RevokeApiKeyResponse {
                    success: true,
                    message: "API key revoked".to_string(),
                }),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(RevokeApiKeyResponse {
                success: false,
                message: "API key not found or already revoked".to_string(),
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RevokeApiKeyResponse {
                success: false,
                message: "Internal database error".to_string(),
            }),
        )
            .into_response(),
    }
}

// ============================================================================
// AUDIT STREAM (GitHub Audit Log Ingestion)
// ============================================================================

pub async fn ingest_audit_stream(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(batch): Json<AuditStreamBatch>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(AuditStreamResponse {
                accepted: 0,
                filtered: 0,
                errors: vec!["Admin access required".to_string()],
            }),
        );
    }

    let org = if let Some(ref org_name) = batch.org_name {
        match state.db.get_org_by_login(org_name).await {
            Ok(Some(o)) => Some(o),
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuditStreamResponse {
                        accepted: 0,
                        filtered: 0,
                        errors: vec![format!("Organization '{}' not found", org_name)],
                    }),
                );
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuditStreamResponse {
                        accepted: 0,
                        filtered: 0,
                        errors: vec![e.to_string()],
                    }),
                );
            }
        }
    } else {
        None
    };

    let mut governance_events = Vec::new();
    let mut filtered = 0;

    for entry in batch.entries {
        if !is_relevant_audit_action(&entry.action) {
            tracing::debug!("Filtered non-relevant audit action: {}", entry.action);
            filtered += 1;
            continue;
        }

        let event_org_id = org.as_ref().map(|o| o.id.clone());
        let event_repo_id = get_repo_id_for_audit_entry(&state, &entry, event_org_id.as_deref()).await;

        let delivery_id = make_audit_delivery_id(&entry, event_org_id.as_deref());

        let (target, old_value, new_value) = extract_audit_changes(&entry);

        let event = GovernanceEvent {
            id: Uuid::new_v4().to_string(),
            org_id: event_org_id,
            repo_id: event_repo_id,
            delivery_id,
            event_type: entry.action.clone(),
            actor_login: entry.actor.clone(),
            target,
            old_value,
            new_value,
            payload: serde_json::to_value(&entry).unwrap_or(serde_json::Value::Null),
            created_at: entry.timestamp,
        };

        governance_events.push(event);
    }

    let (accepted, errors) = match state.db.insert_governance_events_batch(&governance_events).await {
        Ok(result) => result,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuditStreamResponse {
                    accepted: 0,
                    filtered,
                    errors: vec![e.to_string()],
                }),
            );
        }
    };

    tracing::info!(
        "Ingested {} governance events, filtered {} (org={})",
        accepted,
        filtered,
        batch.org_name.as_deref().unwrap_or("unknown")
    );

    (StatusCode::OK, Json(AuditStreamResponse { accepted, filtered, errors }))
}

pub async fn get_governance_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<GovernanceEventFilter>,
) -> impl IntoResponse {
    let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
    let offset = filter.offset as i64;

    let requested_org_id = if let Some(ref org_name) = filter.org_name {
        state.db.get_org_by_login(org_name).await.ok().flatten().map(|o| o.id)
    } else {
        None
    };

    let org_id = if auth_user.role == UserRole::Admin {
        requested_org_id
    } else {
        if let (Some(scoped_org_id), Some(requested_org_id)) =
            (auth_user.org_id.as_deref(), requested_org_id.as_deref())
        {
            if scoped_org_id != requested_org_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(GovernanceEventsResponse { events: vec![] }),
                );
            }
        }
        auth_user.org_id.clone().or(requested_org_id)
    };

    match state.db.get_governance_events(org_id.as_deref(), filter.event_type.as_deref(), limit, offset).await {
        Ok(events) => (StatusCode::OK, Json(GovernanceEventsResponse { events })),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GovernanceEventsResponse { events: vec![] }),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceEventFilter {
    pub org_name: Option<String>,
    pub event_type: Option<String>,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceEventsResponse {
    pub events: Vec<GovernanceEvent>,
}

fn is_relevant_audit_action(action: &str) -> bool {
    RELEVANT_AUDIT_ACTIONS.contains(&action)
}

fn make_audit_delivery_id(entry: &GitHubAuditLogEntry, org_id: Option<&str>) -> String {
    let digest_input = serde_json::json!({
        "org_id": org_id,
        "timestamp": entry.timestamp,
        "action": entry.action,
        "actor": entry.actor,
        "repo": entry.repo,
        "repository": entry.repository,
        "repository_id": entry.repository_id,
        "team": entry.team,
        "user": entry.user,
        "data": entry.data,
    });

    let bytes = serde_json::to_vec(&digest_input).unwrap_or_default();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    format!("audit-{}-{}", entry.timestamp, hash)
}

async fn get_repo_id_for_audit_entry(state: &Arc<AppState>, entry: &GitHubAuditLogEntry, _org_id: Option<&str>) -> Option<String> {
    let repo_name = entry.repo.as_ref()
        .or(entry.repository.as_ref())?;

    state.db.get_repo_by_full_name(repo_name).await.ok().flatten().map(|r| r.id)
}

fn extract_audit_changes(entry: &GitHubAuditLogEntry) -> (Option<String>, Option<serde_json::Value>, Option<serde_json::Value>) {
    let target = entry.repo.clone()
        .or_else(|| entry.team.clone())
        .or_else(|| entry.user.clone());

    let (old_value, new_value) = if let Some(ref data) = entry.data {
        let old = data.get("old").cloned();
        let new = data.get("new").cloned();
        (old, new)
    } else {
        (None, None)
    };

    (target, old_value, new_value)
}

// ============================================================================
// JOB QUEUE MANAGEMENT ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct JobMetricsResponse {
    pub worker_id: String,
    pub metrics: JobMetrics,
}

pub async fn get_job_metrics(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Admin access required".to_string(),
                code: "FORBIDDEN".to_string(),
            }),
        )
            .into_response();
    }

    match state.db.get_job_metrics().await {
        Ok(metrics) => (
            StatusCode::OK,
            Json(JobMetricsResponse {
                worker_id: state.worker_id.clone(),
                metrics,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to get job metrics".to_string(),
                code: "INTERNAL_ERROR".to_string(),
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeadJobsResponse {
    pub jobs: Vec<Job>,
    pub total: usize,
}

pub async fn get_dead_jobs(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeadJobsQuery>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(DeadJobsResponse { jobs: vec![], total: 0 }),
        );
    }

    let limit = params.limit.unwrap_or(50);

    match state.db.get_dead_jobs(limit).await {
        Ok(jobs) => {
            let total = jobs.len();
            (StatusCode::OK, Json(DeadJobsResponse { jobs, total }))
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DeadJobsResponse { jobs: vec![], total: 0 }),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeadJobsQuery {
    pub limit: Option<i64>,
}

pub async fn retry_dead_job(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    match state.db.retry_dead_job(&job_id).await {
        Ok(()) => {
            tracing::info!(
                job_id = %job_id,
                admin = %auth_user.client_id,
                "Dead job queued for retry"
            );
            (
                StatusCode::OK,
                Json(json!({"success": true, "job_id": job_id})),
            )
        }
        Err(e) => {
            let status = match e {
                DbError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({"error": sanitize_db_error(&e)})),
            )
        }
    }
}

// ============================================================================
// PULL REQUEST MERGES EVIDENCE ENDPOINT
// ============================================================================

pub async fn list_pr_merges(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PrMergeEvidenceQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_pr_merge_evidence(
            auth_user.org_id.as_deref(),
            query.org_name.as_deref(),
            query.repo_full_name.as_deref(),
            query.merged_by.as_deref(),
            limit,
            offset,
        )
        .await
    {
        Ok((entries, total)) => (
            StatusCode::OK,
            Json(PrMergeEvidenceResponse { entries, total }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

// ============================================================================
// ADMIN AUDIT LOG ENDPOINT
// ============================================================================

pub async fn list_admin_audit_log(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<AdminAuditLogQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let limit = if query.limit == 0 { 50 } else { query.limit } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_admin_audit_logs(
            query.actor.as_deref(),
            query.action.as_deref(),
            limit,
            offset,
        )
        .await
    {
        Ok((entries, total)) => (
            StatusCode::OK,
            Json(AdminAuditLogResponse { entries, total }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

// ============================================================================
// GDPR — T2 (POST /users/:login/erase, GET /users/:login/export)
// ============================================================================

pub async fn erase_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(login): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let login = login.trim().to_string();
    if login.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "login cannot be empty" })),
        )
            .into_response();
    }

    let org_scope = auth_user.org_id.as_deref();
    match state.db.erase_user_data(&login, org_scope).await {
        Ok((client_count, github_count)) => {
            let status = erase_result_status(client_count, github_count);
            if status == StatusCode::NOT_FOUND {
                return (
                    status,
                    Json(serde_json::json!({ "error": "User not found in visible scope" })),
                )
                    .into_response();
            }

            // Append-only audit log entry for the erasure
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "gdpr_erase".to_string(),
                target_type: Some("user".to_string()),
                target_id: Some(login.clone()),
                metadata: serde_json::json!({
                    "client_events_erased": client_count,
                    "github_events_erased": github_count,
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write GDPR audit log entry");
            }

            tracing::info!(
                actor = %auth_user.client_id,
                login = %login,
                client_events = client_count,
                github_events = github_count,
                "GDPR erasure completed"
            );

            (
                status,
                Json(EraseUserResponse {
                    user_login: login,
                    client_events_erased: client_count,
                    github_events_erased: github_count,
                    erased_at: chrono::Utc::now().timestamp_millis(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, login = %login, "GDPR erase failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn export_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(login): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let login = login.trim().to_string();
    if login.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "login cannot be empty" })),
        )
            .into_response();
    }

    let org_scope = auth_user.org_id.as_deref();
    match state.db.export_user_data(&login, org_scope).await {
        Ok(events) => {
            let status = export_result_status(events.len());
            if status == StatusCode::NOT_FOUND {
                return (
                    status,
                    Json(serde_json::json!({ "error": "User not found in visible scope" })),
                )
                    .into_response();
            }

            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "gdpr_export".to_string(),
                target_type: Some("user".to_string()),
                target_id: Some(login.clone()),
                metadata: serde_json::json!({ "event_count": events.len() }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write GDPR export audit log entry");
            }

            let total = events.len();
            (
                status,
                Json(ExportUserResponse {
                    user_login: login,
                    events,
                    total,
                    exported_at: chrono::Utc::now().timestamp_millis(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, login = %login, "GDPR export failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// CLIENT SESSIONS — T3.A (GET /clients)
// ============================================================================

pub async fn get_clients(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let org_id = auth_user.org_id.as_deref();

    match state.db.get_client_sessions(org_id).await {
        Ok(sessions) => {
            let total = sessions.len();
            (
                StatusCode::OK,
                Json(ClientSessionsResponse { sessions, total }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch client sessions");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// IDENTITY ALIASES — T3.B (POST /identities/aliases, GET /identities/aliases)
// ============================================================================

pub async fn create_identity_alias(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateIdentityAliasRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let canonical = payload.canonical.trim().to_string();
    let alias = payload.alias.trim().to_string();

    if canonical.is_empty() || alias.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "canonical and alias cannot be empty" })),
        )
            .into_response();
    }
    if canonical == alias {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "canonical and alias must be different" })),
        )
            .into_response();
    }

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        payload.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(org_id) => org_id,
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    match state.db.create_identity_alias(&canonical, &alias, org_id.as_deref()).await {
        Ok(created) => (
            if created { StatusCode::CREATED } else { StatusCode::OK },
            Json(CreateIdentityAliasResponse {
                canonical_login: canonical,
                alias_login: alias,
                created,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create identity alias");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn list_identity_aliases(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let org_id = auth_user.org_id.as_deref();

    match state.db.list_identity_aliases(org_id).await {
        Ok(aliases) => (StatusCode::OK, Json(aliases)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list identity aliases");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// PURE SCOPE HELPERS — unit-testable without a database
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrgScopeError {
    BadRequest,
    NotFound,
    Forbidden,
    Internal,
}

fn org_scope_status(error: OrgScopeError) -> StatusCode {
    match error {
        OrgScopeError::BadRequest => StatusCode::BAD_REQUEST,
        OrgScopeError::NotFound => StatusCode::NOT_FOUND,
        OrgScopeError::Forbidden => StatusCode::FORBIDDEN,
        OrgScopeError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Resolves effective `org_id` and validates API key scope constraints.
///
/// | auth_org_id | org_was_provided | resolved_org_id | Result                      |
/// |-------------|------------------|-----------------|-----------------------------|
/// | None        | false            | —               | Err(BadRequest)             |
/// | *           | true             | None            | Err(NotFound)               |
/// | Some(a)     | true             | Some(b) a ≠ b   | Err(Forbidden)              |
/// | Some(a)     | true             | Some(a)         | Ok(Some(a))                 |
/// | Some(a)     | false            | —               | Ok(Some(a)) implicit scope  |
/// | None        | true             | Some(b)         | Ok(Some(b)) global admin    |
fn check_org_scope_match(
    auth_org_id: Option<&str>,
    org_was_provided: bool,
    resolved_org_id: Option<&str>,
) -> Result<Option<String>, OrgScopeError> {
    if !org_was_provided && auth_org_id.is_none() {
        return Err(OrgScopeError::BadRequest);
    }
    if org_was_provided && resolved_org_id.is_none() {
        return Err(OrgScopeError::NotFound);
    }
    let effective = resolved_org_id.or(auth_org_id);
    if let (Some(scoped), Some(eff)) = (auth_org_id, effective) {
        if scoped != eff {
            return Err(OrgScopeError::Forbidden);
        }
    }
    Ok(effective.map(|s| s.to_string()))
}

async fn resolve_and_check_org_scope(
    state: &Arc<AppState>,
    auth_org_id: Option<&str>,
    requested_org_name: Option<&str>,
    require_org_for_global: bool,
) -> Result<Option<String>, OrgScopeError> {
    let resolved_org_id = match requested_org_name {
        Some(org_name) => match state.db.get_org_by_login(org_name).await {
            Ok(Some(org)) => Some(org.id),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, org_name = %org_name, "Failed to resolve org scope");
                return Err(OrgScopeError::Internal);
            }
        },
        None => None,
    };

    if !require_org_for_global && requested_org_name.is_none() && auth_org_id.is_none() {
        return Ok(None);
    }

    check_org_scope_match(
        auth_org_id,
        requested_org_name.is_some(),
        resolved_org_id.as_deref(),
    )
}

/// Returns the HTTP status for an erase operation given how many rows were
/// updated. When both counts are zero the user was not visible in the org
/// scope, so we return 404 — privacy-preserving (indistinguishable from
/// "user does not exist").
fn erase_result_status(client_count: i64, github_count: i64) -> StatusCode {
    if client_count == 0 && github_count == 0 {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    }
}

/// Returns the HTTP status for an export operation given how many events were
/// found. When zero, the user was not visible in the org scope → 404.
fn export_result_status(event_count: usize) -> StatusCode {
    if event_count == 0 {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    }
}

// ============================================================================
// CONVERSATIONAL CHAT — POST /chat/ask  (admin-only MVP)
// ============================================================================

const CHAT_SYSTEM_PROMPT: &str = "Eres el asistente de GitGov.\n\
Reglas:\n\
- Responde preguntas sobre: governance Git, commits, pushes, bloqueos, tickets, pipelines, integraciones (GitHub/Jira/Jenkins/GitHub Actions), configuración, troubleshooting y FAQ del proyecto.\n\
- Usa EXCLUSIVAMENTE los datos/contexto provistos por el backend en el campo <data>. NO inventes datos.\n\
- Si los datos están vacíos o son insuficientes, devuelve status=\"insufficient_data\".\n\
- Si la capacidad no existe en el sistema, devuelve status=\"feature_not_available\".\n\
- Si puedes responder con los datos/contexto, devuelve status=\"ok\".\n\
- Responde siempre en el idioma del usuario.\n\
- Sé claro y breve.\n\
- Tu respuesta DEBE ser JSON válido con este esquema exacto:\n\
  {\"status\":\"ok\"|\"insufficient_data\"|\"feature_not_available\"|\"error\",\
\"answer\":\"<string>\",\"missing_capability\":\"<string o null>\",\
\"can_report_feature\":true|false,\"data_refs\":[\"<strings>\"]}\n\
- Si status=ok: answer contiene la respuesta en lenguaje natural, can_report_feature=false.\n\
- Si status=feature_not_available: answer explica qué falta, can_report_feature=true, \
missing_capability describe la capacidad faltante.\n\
- Si status=insufficient_data: answer explica por qué no hay suficientes datos, can_report_feature=false.\n\
- NUNCA devuelvas texto fuera del JSON.";

const PROJECT_KNOWLEDGE_BASE: &[(&str, &[&str], &str)] = &[
    (
        "Integracion GitHub",
        &["github", "webhook", "push", "hmac", "firma", "delivery"],
        "GitHub se integra por POST /webhooks/github. Se valida firma HMAC X-Hub-Signature-256 con GITHUB_WEBHOOK_SECRET. La cabecera X-GitHub-Delivery se usa para idempotencia de eventos.",
    ),
    (
        "Integracion Jenkins",
        &["jenkins", "pipeline", "ci", "build", "correlation", "correlacion"],
        "Jenkins ingresa por POST /integrations/jenkins (Bearer auth). Si JENKINS_WEBHOOK_SECRET está configurado, exige x-gitgov-jenkins-secret. Estado: GET /integrations/jenkins/status. Correlaciones: GET /integrations/jenkins/correlations.",
    ),
    (
        "Integracion Jira",
        &["jira", "ticket", "issue", "correlate", "cobertura", "coverage"],
        "Jira ingresa por POST /integrations/jira (Bearer auth). Si JIRA_WEBHOOK_SECRET está configurado, exige x-gitgov-jira-secret. Estado: GET /integrations/jira/status. Correlación batch: POST /integrations/jira/correlate. Cobertura: GET /integrations/jira/ticket-coverage.",
    ),
    (
        "GitHub Actions",
        &["github actions", "actions", "workflow", "gha"],
        "GitGov no tiene endpoint dedicado para ingestar GitHub Actions por nombre. La trazabilidad CI actual está implementada vía integración Jenkins y correlación commit->pipeline. Para Actions, se requiere capacidad nueva o puente que envíe eventos a endpoint soportado.",
    ),
    (
        "Auth API",
        &["auth", "api key", "bearer", "401", "token"],
        "La autenticación al Control Plane usa Authorization: Bearer <api_key>. El servidor no acepta X-API-Key. Roles y scope por org dependen de la API key almacenada en tabla api_keys.",
    ),
    (
        "Onboarding Admin",
        &["onboarding", "org", "organizacion", "invitation", "invitacion", "admin"],
        "Flujo admin: crear org (POST /orgs), crear invitaciones (POST /org-invitations), validar/aceptar invitación (GET /org-invitations/preview/{token}, POST /org-invitations/accept).",
    ),
    (
        "Golden Path",
        &["golden path", "flujo", "desktop", "events", "dashboard"],
        "Golden Path crítico: Desktop stage/commit/push -> /events -> PostgreSQL -> Dashboard sin 401. Cualquier cambio en auth/outbox/handlers/dashboard debe preservar ese flujo.",
    ),
    (
        "Deploy EC2",
        &["ec2", "deploy", "aws", "systemd", "nginx", "restart"],
        "En producción actual, backend corre en EC2 con systemd y Nginx. Para ver cambios nuevos del servidor se debe desplegar binario/servicio actualizado y reiniciar systemd; si no, endpoints nuevos no existen en runtime.",
    ),
    (
        "Rate Limits",
        &["rate", "429", "limit", "throttle"],
        "El servidor aplica rate limits por endpoint con middlewares en memoria. Variables GITGOV_RATE_LIMIT_* controlan req/min para /events, audit-stream, jenkins, jira y admin.",
    ),
    (
        "Datos y retencion",
        &["retencion", "retention", "compliance", "5 years", "auditoria"],
        "La retención de auditoría está gobernada por AUDIT_RETENTION_DAYS con mínimo legal de 1825 días. client_sessions tiene retención separada por CLIENT_SESSION_RETENTION_DAYS (fallback DATA_RETENTION_DAYS).",
    ),
    (
        "Timezone audit trail",
        &["timezone", "zona horaria", "timestamp", "utc"],
        "Los timestamps se almacenan en UTC y se visualizan en zona horaria seleccionada por el usuario en Settings para trazabilidad de auditoría.",
    ),
    (
        "Chatbot capacidades",
        &["chatbot", "chat", "faq", "feature", "bot"],
        "El chatbot combina consultas analíticas SQL en tiempo real y modo conocimiento del proyecto. Si falta capacidad, responde feature_not_available y puede habilitar reporte de feature.",
    ),
];

#[derive(Debug, serde::Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, serde::Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, serde::Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
    response_mime_type: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    system_instruction: GeminiSystemInstruction,
    contents: Vec<GeminiContent>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

/// Pattern matching for the 3 supported queries.
enum ChatQuery {
    PushesNoTicket,
    BlockedPushesMonth,
    UserCommitsRange { user: String, start_ms: i64, end_ms: i64 },
}

fn detect_query(question: &str) -> Option<ChatQuery> {
    let q = question.to_lowercase();

    // Q1: push a main esta semana sin ticket de Jira
    if (q.contains("push") || q.contains("empujo") || q.contains("empujaron"))
        && (q.contains("main") || q.contains("principal"))
        && (q.contains("semana") || q.contains("week") || q.contains("últimos 7") || q.contains("last 7"))
        && (q.contains("ticket") || q.contains("jira") || q.contains("sin ticket") || q.contains("without ticket"))
    {
        return Some(ChatQuery::PushesNoTicket);
    }

    // Q2: pushes bloqueados este mes
    if (q.contains("bloqueado") || q.contains("bloqueados") || q.contains("blocked"))
        && (q.contains("push") || q.contains("pushes"))
        && (q.contains("mes") || q.contains("month") || q.contains("este mes") || q.contains("this month"))
    {
        return Some(ChatQuery::BlockedPushesMonth);
    }

    // Q3: commits de {usuario} entre {fecha_inicio} y {fecha_fin}
    if q.contains("commit") {
        // Extract user login — look for "de {word}" or "by {word}" or "del usuario {word}"
        let user_re = Regex::new(r"(?:de |by |del usuario |of user )([a-z0-9_\-\.]+)").ok()?;
        let user = user_re
            .captures(&q)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())?;

        // Extract date range — look for "entre YYYY-MM-DD y YYYY-MM-DD" or "from ... to ..."
        let date_re = Regex::new(
            r"(?:entre|from|desde)\s+(\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4})\s+(?:y|and|to|hasta)\s+(\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4})"
        ).ok()?;

        let (start_ms, end_ms) = if let Some(caps) = date_re.captures(&q) {
            let parse_date = |s: &str| -> Option<i64> {
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .or_else(|_| chrono::NaiveDate::parse_from_str(s, "%d/%m/%Y"))
                    .ok()
                    .map(|d| {
                        d.and_hms_opt(0, 0, 0)
                            .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).timestamp_millis())
                            .unwrap_or(0)
                    })
            };
            let s = parse_date(caps.get(1)?.as_str())?;
            let e = parse_date(caps.get(2)?.as_str())?;
            (s, e)
        } else {
            // Default to last 30 days if no dates found
            let now = chrono::Utc::now().timestamp_millis();
            let thirty_days_ago = now - 30 * 24 * 60 * 60 * 1000;
            (thirty_days_ago, now)
        };

        return Some(ChatQuery::UserCommitsRange { user, start_ms, end_ms });
    }

    None
}

fn build_project_knowledge_payload(question: &str) -> serde_json::Value {
    let q = question.to_lowercase();
    let mut ranked: Vec<(i32, &str, &str)> = Vec::new();
    for (title, keywords, content) in PROJECT_KNOWLEDGE_BASE {
        let mut score = 0;
        for kw in *keywords {
            if q.contains(kw) {
                score += 2;
            }
        }
        if score > 0 {
            ranked.push((score, *title, *content));
        }
    }

    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    let selected: Vec<serde_json::Value> = if ranked.is_empty() {
        PROJECT_KNOWLEDGE_BASE
            .iter()
            .take(4)
            .map(|(title, _keywords, content)| {
                serde_json::json!({ "title": title, "content": content })
            })
            .collect()
    } else {
        ranked
            .into_iter()
            .take(8)
            .map(|(_score, title, content)| serde_json::json!({ "title": title, "content": content }))
            .collect()
    };

    serde_json::json!({
        "mode": "project_knowledge",
        "snippets": selected
    })
}

async fn call_llm(
    http_client: &reqwest::Client,
    api_key: &str,
    model: &str,
    question: &str,
    data: &serde_json::Value,
) -> Result<ChatAskResponse, String> {
    let user_message = format!(
        "Pregunta: {}\n<data>{}</data>",
        question,
        serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string())
    );

    let req_body = GeminiRequest {
        system_instruction: GeminiSystemInstruction {
            parts: vec![GeminiPart {
                text: CHAT_SYSTEM_PROMPT.to_string(),
            }],
        },
        contents: vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart { text: user_message }],
        }],
        generation_config: GeminiGenerationConfig {
            temperature: 0.2,
            max_output_tokens: 1024,
            response_mime_type: "application/json".to_string(),
        },
    };

    let response = http_client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        ))
        .header("content-type", "application/json")
        .json(&req_body)
        .send()
        .await
        .map_err(|e| format!("LLM network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM API returned {}: {}", status, &body[..body.len().min(200)]));
    }

    let gemini_resp: GeminiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

    let text = gemini_resp
        .candidates
        .unwrap_or_default()
        .into_iter()
        .find_map(|c| c.content)
        .and_then(|content| content.parts)
        .unwrap_or_default()
        .into_iter()
        .find_map(|p| p.text)
        .ok_or_else(|| "LLM response had no text content".to_string())?;

    // Strip markdown code fences if present
    let json_str = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<ChatAskResponse>(json_str)
        .map_err(|e| format!("Failed to parse LLM JSON: {} — raw: {}", e, &json_str[..json_str.len().min(300)]))
}

pub async fn chat_ask(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatAskRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(ChatAskResponse {
                status: "error".to_string(),
                answer: "Admin access required".to_string(),
                missing_capability: None,
                can_report_feature: false,
                data_refs: vec![],
            }),
        );
    }

    let question = payload.question.trim().to_string();
    if question.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ChatAskResponse {
                status: "error".to_string(),
                answer: "La pregunta no puede estar vacía".to_string(),
                missing_capability: None,
                can_report_feature: false,
                data_refs: vec![],
            }),
        );
    }

    let Some(api_key) = state.llm_api_key.as_deref() else {
        tracing::warn!("GEMINI_API_KEY not configured; returning feature_not_available");
        return (
            StatusCode::OK,
            Json(ChatAskResponse {
                status: "feature_not_available".to_string(),
                answer: "El asistente conversacional no está configurado en este servidor. Configura GEMINI_API_KEY para activarlo.".to_string(),
                missing_capability: Some("llm_integration".to_string()),
                can_report_feature: true,
                data_refs: vec![],
            }),
        );
    };

    let org_name = payload.org_name.as_deref();
    let scoped_org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        org_name,
        false,
    )
    .await
    {
        Ok(org_id) => org_id,
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (
                org_scope_status(err),
                Json(ChatAskResponse {
                    status: "error".to_string(),
                    answer: error.to_string(),
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec![],
                }),
            );
        }
    };

    // Run query engine
    let query = detect_query(&question);

    let (data, data_refs) = match query {
        Some(ChatQuery::PushesNoTicket) => {
            match state.db.chat_query_pushes_no_ticket(scoped_org_id.as_deref()).await {
                Ok(rows) => {
                    let refs = vec!["github_events".to_string(), "commit_ticket_correlations".to_string()];
                    (serde_json::json!({ "pushes_to_main_no_ticket": rows }), refs)
                }
                Err(e) => {
                    tracing::error!("chat_query_pushes_no_ticket error: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(ChatAskResponse {
                        status: "error".to_string(),
                        answer: "Error consultando la base de datos".to_string(),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![],
                    }));
                }
            }
        }
        Some(ChatQuery::BlockedPushesMonth) => {
            match state.db.chat_query_blocked_pushes_month(scoped_org_id.as_deref()).await {
                Ok(count) => {
                    let refs = vec!["client_events".to_string()];
                    (serde_json::json!({ "blocked_pushes_this_month": count }), refs)
                }
                Err(e) => {
                    tracing::error!("chat_query_blocked_pushes_month error: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(ChatAskResponse {
                        status: "error".to_string(),
                        answer: "Error consultando la base de datos".to_string(),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![],
                    }));
                }
            }
        }
        Some(ChatQuery::UserCommitsRange { ref user, start_ms, end_ms }) => {
            match state
                .db
                .chat_query_user_commits_range(user, start_ms, end_ms, scoped_org_id.as_deref())
                .await
            {
                Ok(rows) => {
                    let refs = vec!["client_events".to_string()];
                    (serde_json::json!({
                        "user": user,
                        "start_ms": start_ms,
                        "end_ms": end_ms,
                        "commits": rows
                    }), refs)
                }
                Err(e) => {
                    tracing::error!("chat_query_user_commits_range error: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(ChatAskResponse {
                        status: "error".to_string(),
                        answer: "Error consultando la base de datos".to_string(),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![],
                    }));
                }
            }
        }
        None => {
            let refs = vec!["project_docs_kb".to_string()];
            (build_project_knowledge_payload(&question), refs)
        }
    };

    // Call LLM
    match call_llm(&state.http_client, api_key, &state.llm_model, &question, &data).await {
        Ok(mut resp) => {
            resp.data_refs = data_refs;
            (StatusCode::OK, Json(resp))
        }
        Err(e) => {
            tracing::error!("LLM call failed: {}", e);
            (StatusCode::OK, Json(ChatAskResponse {
                status: "error".to_string(),
                answer: "El asistente no pudo generar una respuesta. Intenta de nuevo.".to_string(),
                missing_capability: None,
                can_report_feature: false,
                data_refs,
            }))
        }
    }
}

// ============================================================================
// FEATURE REQUESTS — POST /feature-requests
// ============================================================================

pub async fn create_feature_request_handler(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FeatureRequestInput>,
) -> impl IntoResponse {
    let question = payload.question.trim().to_string();
    if question.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "question is required" })),
        );
    }

    let requested_by = auth_user.client_id.as_str();

    let effective_org_id = if let Some(scoped_org_id) = auth_user.org_id.as_deref() {
        if let Some(ref requested_org_id) = payload.org_id {
            if requested_org_id != scoped_org_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "error": "org_id is outside API key scope" })),
                );
            }
        }
        Some(scoped_org_id.to_string())
    } else {
        payload.org_id.clone()
    };

    let sanitized_payload = FeatureRequestInput {
        question: payload.question.clone(),
        missing_capability: payload.missing_capability.clone(),
        org_id: effective_org_id,
        user_login: None,
        metadata: payload.metadata.clone(),
    };

    let id = match state.db.create_feature_request(&sanitized_payload, requested_by).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("create_feature_request db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to save feature request" })),
            );
        }
    };

    // Optional webhook notification
    if let Some(ref webhook_url) = state.feature_request_webhook_url {
        let body = serde_json::json!({
            "id": &id,
            "requested_by": requested_by,
            "question": &question,
            "missing_capability": &sanitized_payload.missing_capability,
            "org_id": &sanitized_payload.org_id,
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });
        let client = state.http_client.clone();
        let url = webhook_url.clone();
        tokio::spawn(async move {
            if let Err(e) = client.post(&url).json(&body).send().await {
                tracing::warn!("feature_request webhook failed: {}", e);
            }
        });
    }

    tracing::info!(id = %id, requested_by = %requested_by, "Feature request created");
    (StatusCode::CREATED, Json(serde_json::json!({ "id": id, "status": "new" })))
}

#[cfg(test)]
mod tests {
    use super::{
        check_org_scope_match, erase_result_status, export_result_status, extract_final_approvers,
        extract_ticket_ids, is_relevant_audit_action, make_audit_delivery_id, validate_github_signature,
        GitHubPrReview, GitHubPrReviewUser, OrgScopeError,
    };
    use axum::http::StatusCode;
    use crate::models::GitHubAuditLogEntry;
    use hmac::Mac;
    use sha2::Sha256;

    fn sign(secret: &str, body: &[u8]) -> String {
        let mut mac = <hmac::Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn validates_correct_github_signature_for_raw_body() {
        let secret = "top-secret";
        let body = br#"{"ref":"refs/heads/main","forced":false}"#;
        let signature = sign(secret, body);

        assert!(validate_github_signature(secret, body, &signature));
    }

    #[test]
    fn rejects_invalid_or_malformed_signature() {
        let secret = "top-secret";
        let body = br#"{"a":1}"#;

        assert!(!validate_github_signature(secret, body, "sha256=deadbeef"));
        assert!(!validate_github_signature(secret, body, "deadbeef"));
        assert!(!validate_github_signature(secret, body, "sha256=not-hex"));
    }

    #[test]
    fn raw_body_bytes_matter_for_signature_validation() {
        let secret = "top-secret";
        let compact = br#"{"a":1}"#;
        let pretty = b"{\n  \"a\": 1\n}";
        let signature = sign(secret, compact);

        assert!(validate_github_signature(secret, compact, &signature));
        assert!(!validate_github_signature(secret, pretty, &signature));
    }

    #[test]
    fn audit_action_filter_is_exact_and_rejects_prefix_overmatch() {
        assert!(is_relevant_audit_action("repo.permissions_granted"));
        assert!(!is_relevant_audit_action("repo.delete"));
        assert!(!is_relevant_audit_action("protected_branch.unknown_new_event"));
    }

    #[test]
    fn audit_delivery_id_is_deterministic_for_same_entry() {
        let entry = GitHubAuditLogEntry {
            timestamp: 1_700_000_000_000,
            action: "repo.permissions_granted".to_string(),
            actor: Some("alice".to_string()),
            actor_location: None,
            org: Some("acme".to_string()),
            repo: Some("acme/app".to_string()),
            repository: None,
            repository_id: Some(123),
            user: Some("bob".to_string()),
            team: None,
            data: Some(serde_json::json!({"new": "write"})),
            created_at: None,
        };

        let id1 = make_audit_delivery_id(&entry, Some("org-1"));
        let id2 = make_audit_delivery_id(&entry, Some("org-1"));
        let id3 = make_audit_delivery_id(&entry, Some("org-2"));

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn extracts_ticket_ids_from_commit_message_and_branch() {
        let tickets = extract_ticket_ids(&[
            "feat: JIRA-123 implement pipeline health",
            "feature/JIRA-123-ci-widget",
        ]);

        assert_eq!(tickets, vec!["JIRA-123"]);
    }

    #[test]
    fn extracts_multiple_unique_ticket_ids_preserving_first_seen_order() {
        let tickets = extract_ticket_ids(&[
            "fix: PROJ-12 and OPS-9",
            "refs OPS-9 plus SEC-101",
            "PROJ-12 duplicate mention",
        ]);

        assert_eq!(tickets, vec!["PROJ-12", "OPS-9", "SEC-101"]);
    }

    #[test]
    fn ignores_invalid_ticket_like_strings() {
        let tickets = extract_ticket_ids(&[
            "jira-123 lowercase should not match",
            "A-1 too short project key",
            "NOSEP123 missing dash",
            "ABC- not complete",
        ]);

        assert!(tickets.is_empty());
    }

    #[test]
    fn pr_approvers_take_latest_review_state_per_user() {
        let reviews = vec![
            GitHubPrReview {
                state: Some("APPROVED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "alice".to_string(),
                }),
            },
            GitHubPrReview {
                state: Some("CHANGES_REQUESTED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "alice".to_string(),
                }),
            },
            GitHubPrReview {
                state: Some("COMMENTED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "bob".to_string(),
                }),
            },
            GitHubPrReview {
                state: Some("APPROVED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "carol".to_string(),
                }),
            },
        ];

        let approvers = extract_final_approvers(&reviews);
        assert_eq!(approvers, vec!["carol"]);
    }

    #[test]
    fn pr_approvers_are_sorted_and_unique() {
        let reviews = vec![
            GitHubPrReview {
                state: Some("APPROVED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "zoe".to_string(),
                }),
            },
            GitHubPrReview {
                state: Some("APPROVED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "anna".to_string(),
                }),
            },
            GitHubPrReview {
                state: Some("APPROVED".to_string()),
                user: Some(GitHubPrReviewUser {
                    login: "zoe".to_string(),
                }),
            },
        ];

        let approvers = extract_final_approvers(&reviews);
        assert_eq!(approvers, vec!["anna", "zoe"]);
    }

    // ── Scope enforcement: create_identity_alias ──────────────────────────────

    #[test]
    fn alias_scope_global_admin_no_org_name_returns_bad_request() {
        // A global admin key (no org_id) must always supply org_name.
        assert_eq!(
            check_org_scope_match(None, false, None),
            Err(OrgScopeError::BadRequest)
        );
    }

    #[test]
    fn alias_scope_org_name_not_found_in_db_returns_not_found() {
        // org_name was provided but the DB found no matching org → 404.
        assert_eq!(
            check_org_scope_match(Some("uuid-rimac"), true, None),
            Err(OrgScopeError::NotFound)
        );
        // Same for global admin keys.
        assert_eq!(
            check_org_scope_match(None, true, None),
            Err(OrgScopeError::NotFound)
        );
    }

    #[test]
    fn alias_scope_scoped_admin_wrong_org_returns_forbidden() {
        // Scoped admin key for org A cannot create aliases in org B.
        assert_eq!(
            check_org_scope_match(Some("uuid-a"), true, Some("uuid-b")),
            Err(OrgScopeError::Forbidden)
        );
    }

    #[test]
    fn alias_scope_scoped_admin_no_org_name_uses_key_org() {
        // Scoped admin omits org_name → implicit scope from key.
        assert_eq!(
            check_org_scope_match(Some("uuid-rimac"), false, None),
            Ok(Some("uuid-rimac".to_string()))
        );
    }

    #[test]
    fn alias_scope_scoped_admin_matching_org_returns_ok() {
        // Scoped admin + org_name resolves to the same org as the key → OK.
        assert_eq!(
            check_org_scope_match(Some("uuid-rimac"), true, Some("uuid-rimac")),
            Ok(Some("uuid-rimac".to_string()))
        );
    }

    #[test]
    fn alias_scope_global_admin_with_valid_org_name_resolves() {
        // Global admin + valid org_name → use resolved org_id.
        assert_eq!(
            check_org_scope_match(None, true, Some("uuid-rimac")),
            Ok(Some("uuid-rimac".to_string()))
        );
    }

    // ── Scope enforcement: erase_user ─────────────────────────────────────────

    #[test]
    fn erase_scope_out_of_scope_user_is_not_found() {
        // When a scoped admin erases a user that has no events in their org,
        // the DB returns (0, 0). We return 404 — privacy-preserving: the caller
        // cannot distinguish "user exists in another org" from "user not found".
        assert_eq!(erase_result_status(0, 0), StatusCode::NOT_FOUND);
    }

    #[test]
    fn erase_scope_in_scope_user_returns_ok() {
        assert_eq!(erase_result_status(5, 0), StatusCode::OK);
        assert_eq!(erase_result_status(0, 3), StatusCode::OK);
        assert_eq!(erase_result_status(2, 7), StatusCode::OK);
    }

    // ── Scope enforcement: export_user ────────────────────────────────────────

    #[test]
    fn export_scope_out_of_scope_user_is_not_found() {
        // No events visible for the scoped admin → 404 (privacy-preserving).
        assert_eq!(export_result_status(0), StatusCode::NOT_FOUND);
    }

    #[test]
    fn export_scope_in_scope_user_returns_ok() {
        assert_eq!(export_result_status(1), StatusCode::OK);
        assert_eq!(export_result_status(100), StatusCode::OK);
    }
}
