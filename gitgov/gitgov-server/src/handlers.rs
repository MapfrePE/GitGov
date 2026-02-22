use crate::auth::{require_admin, AuthUser};
use crate::db::{Database, DbError, JobMetrics, Job};
use crate::models::*;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use hmac::Mac;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;
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

    match state.db.get_noncompliance_signals(
        filter.org_name.as_deref(),
        filter.confidence.as_deref(),
        filter.status.as_deref(),
        filter.signal_type.as_deref(),
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
    State(state): State<Arc<AppState>>,
    Path(signal_id): Path<String>,
    Json(payload): Json<UpdateSignalRequest>,
) -> impl IntoResponse {
    match state.db.update_signal_status(
        &signal_id,
        &payload.status,
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
    State(state): State<Arc<AppState>>,
    Path(org_name): Path<String>,
) -> impl IntoResponse {
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
        &payload.decided_by,
        payload.notes.as_deref(),
        payload.evidence.clone(),
    ).await {
        Ok(decision_id) => {
            tracing::info!(
                violation_id = %violation_id,
                decision_type = %payload.decision_type,
                decided_by = %payload.decided_by,
                admin = %auth_user.client_id,
                "Violation decision added"
            );
            (
                StatusCode::OK,
                Json(json!({
                    "decision_id": decision_id,
                    "violation_id": violation_id,
                    "decision_type": payload.decision_type,
                    "decided_by": payload.decided_by
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
    Extension(_auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(violation_id): Path<String>,
) -> impl IntoResponse {
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
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExportRequest>,
) -> impl IntoResponse {
    let org_id = if let Some(ref org_name) = payload.org_name {
        state.db.get_org_by_login(org_name).await.ok().flatten().map(|o| o.id)
    } else {
        None
    };

    let filter = EventFilter {
        start_date: payload.start_date,
        end_date: payload.end_date,
        ..Default::default()
    };

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
        org_id,
        exported_by: "api".to_string(),
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
    Json(payload): Json<serde_json::Value>,
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
            if !validate_github_signature(secret, &payload, sig) {
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

fn validate_github_signature(secret: &str, payload: &serde_json::Value, signature: &str) -> bool {
    let payload_bytes = match serde_json::to_vec(payload) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac = match <hmac::Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(&payload_bytes);
    let result = mac.finalize();
    let computed = format!("sha256={}", hex::encode(result.into_bytes()));

    signature == computed
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
    State(state): State<Arc<AppState>>,
    Json(batch): Json<ClientEventBatch>,
) -> impl IntoResponse {
    let mut events = Vec::new();

    for input in batch.events {
        // Get org and repo IDs
        let org_id = if let Some(ref org_name) = input.org_name {
            state.db.get_org_by_login(org_name).await
                .ok()
                .flatten()
                .map(|o| o.id)
        } else {
            None
        };

        let repo_id = if let Some(ref repo_full_name) = input.repo_full_name {
            state.db.get_repo_by_full_name(repo_full_name).await
                .ok()
                .flatten()
                .map(|r| r.id)
        } else {
            None
        };

        let event = ClientEvent {
            id: Uuid::new_v4().to_string(),
            org_id,
            repo_id,
            event_uuid: input.event_uuid,
            event_type: ClientEventType::from_str(&input.event_type),
            user_login: input.user_login,
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
    let filter = if auth_user.role != UserRole::Admin {
        EventFilter {
            user_login: Some(auth_user.client_id.clone()),
            limit: if filter.limit == 0 { 100 } else { filter.limit },
            ..filter
        }
    } else {
        EventFilter {
            limit: if filter.limit == 0 { 100 } else { filter.limit },
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
        Ok(stats) => (StatusCode::OK, Json(stats)),
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

        let delivery_id = format!("audit-{}-{}", entry.timestamp, uuid::Uuid::new_v4());

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
    Extension(_auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<GovernanceEventFilter>,
) -> impl IntoResponse {
    let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
    let offset = filter.offset as i64;

    let org_id = if let Some(ref org_name) = filter.org_name {
        state.db.get_org_by_login(org_name).await.ok().flatten().map(|o| o.id)
    } else {
        None
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
    RELEVANT_AUDIT_ACTIONS.iter().any(|relevant| {
        action == *relevant || action.starts_with(&format!("{}.", relevant.split('.').next().unwrap_or("")))
    })
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": sanitize_db_error(&e)})),
            )
        }
    }
}
