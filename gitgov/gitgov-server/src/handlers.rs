use crate::auth::{require_admin, AuthUser};
use crate::db::{Database, DbError, JobMetrics, Job};
use crate::models::*;
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

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub jwt_secret: String,
    pub github_webhook_secret: Option<String>,
    pub jenkins_webhook_secret: Option<String>,
    pub jira_webhook_secret: Option<String>,
    pub start_time: Instant,
    pub worker_id: String,
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(e) = require_admin(&auth_user) {
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

    // If a non-admin requests a specific org, ensure it matches their scoped org (if any)
    if auth_user.role != UserRole::Admin {
        if let (Some(requested_org), Some(scoped_org_id)) = (filter.org_name.as_deref(), auth_user.org_id.as_deref()) {
            match state.db.get_org_by_login(requested_org).await {
                Ok(Some(org)) if org.id == scoped_org_id => {}
                Ok(Some(_)) => {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(SignalsResponse { signals: vec![], total: 0 }),
                    );
                }
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(SignalsResponse { signals: vec![], total: 0 }),
                    );
                }
                Err(_) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SignalsResponse { signals: vec![], total: 0 }),
                    );
                }
            }
        }
    }

    match state.db.get_noncompliance_signals(
        filter.org_name.as_deref(),
        filter.confidence.as_deref(),
        filter.status.as_deref(),
        filter.signal_type.as_deref(),
        filter_user.as_deref(),
        limit,
        offset,
    ).await {
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
    pub limit: usize,
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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
        ..Default::default()
    };
    if auth_user.role != UserRole::Admin {
        filter.user_login = Some(auth_user.client_id.clone());
    }

    let events = match state.db.get_combined_events(&filter).await {
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
        Err(e) => (
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
    let event = GitHubEvent {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        event_type: "push".to_string(),
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
        "Processed push event: {} commits to {} by {}",
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

    for input in batch.events {
        let effective_user_login = if auth_user.role == UserRole::Admin {
            input.user_login.clone()
        } else {
            auth_user.client_id.clone()
        };

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

        let repo = if let Some(ref repo_full_name) = input.repo_full_name {
            state.db.get_repo_by_full_name(repo_full_name).await
                .ok()
                .flatten()
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
        let repo_id = repo.map(|r| r.id);

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
        Ok(response) => (StatusCode::OK, Json(response)),
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
    let filter = if auth_user.role != UserRole::Admin {
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

    match state.db.get_combined_events(&filter).await {
        Ok(events) => (StatusCode::OK, Json(LogsResponse { events, error: None })),
        Err(e) => (
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
    if let Err(_) = require_admin(&auth_user) {
        return (StatusCode::FORBIDDEN, Json(AuditStats::default()));
    }

    match state.db.get_stats().await {
        Ok(mut stats) => {
            stats.pipeline = state.db.get_pipeline_health_stats().await.unwrap_or_default();
            stats.client_events.desktop_pushes_today = match state.db.get_desktop_pushes_today().await {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub stats: AuditStats,
    pub recent_events: Vec<CombinedEvent>,
}

pub async fn get_dashboard(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(_) = require_admin(&auth_user) {
        return (StatusCode::FORBIDDEN, Json(DashboardResponse {
            stats: AuditStats::default(),
            recent_events: vec![],
        }));
    }

    let stats = state.db.get_stats().await.unwrap_or_default();

    let filter = EventFilter {
        limit: 10,
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
    if let Err(_) = require_admin(&auth_user) {
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
        Err(e) => {
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
        Err(e) => (
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
    if let Err(_) = require_admin(&auth_user) {
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
        Err(e) => {
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
        Err(e) => {
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
        Ok(()) => (
            StatusCode::OK,
            Json(PolicyApiResponse {
                version: Some("1.0".to_string()),
                checksum: Some(checksum),
                config: Some(config),
                updated_at: Some(chrono::Utc::now().timestamp_millis()),
                error: None,
            }),
        ),
        Err(e) => (
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
    if let Err(_) = require_admin(&auth_user) {
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

    // Get org ID if provided
    let org_id = if let Some(ref org_name) = payload.org_name {
        state.db.get_org_by_login(org_name).await
            .ok()
            .flatten()
            .map(|o| o.id)
    } else {
        None
    };

    let api_key = Uuid::new_v4().to_string();
    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));

    match state.db.create_api_key(&key_hash, &payload.client_id, org_id.as_deref(), &role).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(ApiKeyResponse {
                api_key: Some(api_key),
                client_id: payload.client_id,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: payload.client_id,
                error: Some("Internal database error".to_string()),
            }),
        ),
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
    if let Err(_) = require_admin(&auth_user) {
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
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GovernanceEventsResponse { events: vec![] }),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceEventFilter {
    pub org_name: Option<String>,
    pub event_type: Option<String>,
    pub limit: usize,
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

async fn get_repo_id_for_audit_entry(state: &Arc<AppState>, entry: &GitHubAuditLogEntry, org_id: Option<&str>) -> Option<String> {
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
    if let Err(_) = require_admin(&auth_user) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    match state.db.get_job_metrics().await {
        Ok(metrics) => {
            (
                StatusCode::OK,
                Json(json!({
                    "worker_id": state.worker_id,
                    "metrics": metrics
                })),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to get job metrics"})),
        ),
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
    if let Err(_) = require_admin(&auth_user) {
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
    if let Err(_) = require_admin(&auth_user) {
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

#[cfg(test)]
mod tests {
    use super::{
        extract_ticket_ids, is_relevant_audit_action, make_audit_delivery_id, validate_github_signature,
    };
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
}
