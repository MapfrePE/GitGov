use crate::auth::{require_admin, AuthUser};
use crate::db::{
    CreatePolicyChangeRequestInput, Database, DbError, Job, JobMetrics,
    ListPolicyChangeRequestsInput, NoncomplianceSignalsQuery, UpsertOrgUserInput,
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
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
use tokio::time::timeout;
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

#[derive(Debug, Clone)]
pub struct StatsCacheEntry {
    pub stats: AuditStats,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct LogsCacheEntry {
    pub events: Vec<CombinedEvent>,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct OrgLookupCacheEntry {
    pub org_id: Option<String>,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct RepoLookupCacheEntry {
    pub repo: Option<Repo>,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheckBlockingScope {
    pub org_pattern: String,
    pub branch_pattern: String,
}

impl PolicyCheckBlockingScope {
    pub fn new(org_pattern: String, branch_pattern: String) -> Self {
        Self {
            org_pattern,
            branch_pattern,
        }
    }

    pub fn matches(&self, org_name: &str, branch: &str) -> bool {
        fn match_pattern(pattern: &str, value: &str) -> bool {
            if pattern == "*" {
                return true;
            }
            match glob::Pattern::new(pattern) {
                Ok(glob_pattern) => glob_pattern.matches(value),
                Err(_) => pattern.eq_ignore_ascii_case(value),
            }
        }

        let normalized_org = org_name.trim().to_ascii_lowercase();
        let normalized_org_pattern = self.org_pattern.trim().to_ascii_lowercase();
        let branch_pattern = self.branch_pattern.trim();
        match_pattern(&normalized_org_pattern, &normalized_org)
            && match_pattern(branch_pattern, branch.trim())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutboxLeaseWaitBuckets {
    pub le_0: u64,
    pub le_250: u64,
    pub le_1000: u64,
    pub le_5000: u64,
    pub gt_5000: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutboxLeaseTelemetrySnapshot {
    pub total_requests: u64,
    pub granted_requests: u64,
    pub denied_requests: u64,
    pub fail_open_disabled_requests: u64,
    pub fail_open_db_error_requests: u64,
    pub ttl_clamped_requests: u64,
    pub wait_clamped_requests: u64,
    pub avg_requested_ttl_ms: u64,
    pub avg_effective_ttl_ms: u64,
    pub avg_wait_ms: u64,
    pub avg_denied_wait_ms: u64,
    pub max_wait_ms: u64,
    pub avg_handler_duration_ms: u64,
    pub max_handler_duration_ms: u64,
    pub wait_buckets: OutboxLeaseWaitBuckets,
    pub last_request_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub enum OutboxLeaseTelemetryMode {
    Granted,
    Denied,
    DisabledFailOpen,
    DbErrorFailOpen,
}

#[derive(Debug, Clone, Copy)]
pub struct OutboxLeaseTelemetryRecord {
    pub mode: OutboxLeaseTelemetryMode,
    pub requested_ttl_ms: u64,
    pub effective_ttl_ms: u64,
    pub wait_ms: u64,
    pub ttl_clamped: bool,
    pub wait_clamped: bool,
    pub handler_duration_ms: u64,
}

#[derive(Debug, Default)]
pub struct OutboxLeaseTelemetry {
    total_requests: u64,
    granted_requests: u64,
    denied_requests: u64,
    fail_open_disabled_requests: u64,
    fail_open_db_error_requests: u64,
    ttl_clamped_requests: u64,
    wait_clamped_requests: u64,
    requested_ttl_sum_ms: u128,
    effective_ttl_sum_ms: u128,
    wait_sum_ms: u128,
    denied_wait_sum_ms: u128,
    max_wait_ms: u64,
    handler_duration_sum_ms: u128,
    max_handler_duration_ms: u64,
    wait_buckets: OutboxLeaseWaitBuckets,
    last_request_at_ms: Option<i64>,
}

impl OutboxLeaseTelemetry {
    fn add_wait_bucket(&mut self, wait_ms: u64) {
        match wait_ms {
            0 => self.wait_buckets.le_0 += 1,
            1..=250 => self.wait_buckets.le_250 += 1,
            251..=1_000 => self.wait_buckets.le_1000 += 1,
            1_001..=5_000 => self.wait_buckets.le_5000 += 1,
            _ => self.wait_buckets.gt_5000 += 1,
        }
    }

    pub fn record(&mut self, record: OutboxLeaseTelemetryRecord) {
        let OutboxLeaseTelemetryRecord {
            mode,
            requested_ttl_ms,
            effective_ttl_ms,
            wait_ms,
            ttl_clamped,
            wait_clamped,
            handler_duration_ms,
        } = record;
        self.total_requests = self.total_requests.saturating_add(1);
        self.requested_ttl_sum_ms = self.requested_ttl_sum_ms.saturating_add(requested_ttl_ms as u128);
        self.effective_ttl_sum_ms = self.effective_ttl_sum_ms.saturating_add(effective_ttl_ms as u128);
        self.wait_sum_ms = self.wait_sum_ms.saturating_add(wait_ms as u128);
        self.max_wait_ms = self.max_wait_ms.max(wait_ms);
        self.handler_duration_sum_ms = self
            .handler_duration_sum_ms
            .saturating_add(handler_duration_ms as u128);
        self.max_handler_duration_ms = self.max_handler_duration_ms.max(handler_duration_ms);
        self.last_request_at_ms = Some(chrono::Utc::now().timestamp_millis());
        self.add_wait_bucket(wait_ms);

        if ttl_clamped {
            self.ttl_clamped_requests = self.ttl_clamped_requests.saturating_add(1);
        }
        if wait_clamped {
            self.wait_clamped_requests = self.wait_clamped_requests.saturating_add(1);
        }

        match mode {
            OutboxLeaseTelemetryMode::Granted => {
                self.granted_requests = self.granted_requests.saturating_add(1);
            }
            OutboxLeaseTelemetryMode::Denied => {
                self.denied_requests = self.denied_requests.saturating_add(1);
                self.denied_wait_sum_ms = self.denied_wait_sum_ms.saturating_add(wait_ms as u128);
            }
            OutboxLeaseTelemetryMode::DisabledFailOpen => {
                self.granted_requests = self.granted_requests.saturating_add(1);
                self.fail_open_disabled_requests = self.fail_open_disabled_requests.saturating_add(1);
            }
            OutboxLeaseTelemetryMode::DbErrorFailOpen => {
                self.granted_requests = self.granted_requests.saturating_add(1);
                self.fail_open_db_error_requests =
                    self.fail_open_db_error_requests.saturating_add(1);
            }
        }
    }

    pub fn snapshot(&self) -> OutboxLeaseTelemetrySnapshot {
        let total = self.total_requests.max(1);
        let denied = self.denied_requests.max(1);
        OutboxLeaseTelemetrySnapshot {
            total_requests: self.total_requests,
            granted_requests: self.granted_requests,
            denied_requests: self.denied_requests,
            fail_open_disabled_requests: self.fail_open_disabled_requests,
            fail_open_db_error_requests: self.fail_open_db_error_requests,
            ttl_clamped_requests: self.ttl_clamped_requests,
            wait_clamped_requests: self.wait_clamped_requests,
            avg_requested_ttl_ms: (self.requested_ttl_sum_ms / total as u128) as u64,
            avg_effective_ttl_ms: (self.effective_ttl_sum_ms / total as u128) as u64,
            avg_wait_ms: (self.wait_sum_ms / total as u128) as u64,
            avg_denied_wait_ms: if self.denied_requests == 0 {
                0
            } else {
                (self.denied_wait_sum_ms / denied as u128) as u64
            },
            max_wait_ms: self.max_wait_ms,
            avg_handler_duration_ms: (self.handler_duration_sum_ms / total as u128) as u64,
            max_handler_duration_ms: self.max_handler_duration_ms,
            wait_buckets: self.wait_buckets.clone(),
            last_request_at_ms: self.last_request_at_ms,
        }
    }
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
    /// Dedicated webhook URLs for policy drift critical alerts.
    /// If empty, drift alerts fallback to `alert_webhook_url`.
    pub drift_alert_webhook_urls: Vec<String>,
    pub strict_actor_match: bool,
    pub reject_synthetic_logins: bool,
    /// Max number of events accepted per `/events` request (0 disables the guard).
    pub events_max_batch: usize,
    /// Gemini API key for conversational chat (env: GEMINI_API_KEY)
    pub llm_api_key: Option<String>,
    /// Gemini model for conversational chat (env: GEMINI_MODEL)
    pub llm_model: String,
    /// Webhook URL to notify on new feature requests (env: FEATURE_REQUEST_WEBHOOK_URL)
    pub feature_request_webhook_url: Option<String>,
    /// In-memory conversational runtime (context, todos, learning) keyed by user/org scope.
    pub conversational_runtime: Arc<Mutex<ConversationalRuntime>>,
    /// Max concurrent LLM chat calls handled by this server node.
    pub chat_llm_semaphore: Arc<Semaphore>,
    /// Max queue wait before rejecting a chat request as busy.
    pub chat_llm_queue_timeout_ms: u64,
    /// Max allowed duration for a single LLM chat call.
    pub chat_llm_timeout_ms: u64,
    /// In-memory short TTL cache for `/stats` response payloads.
    pub stats_cache_ttl: Duration,
    /// Cache keyed by org scope (`org_id` or `__global__`).
    pub stats_cache: Arc<Mutex<HashMap<String, StatsCacheEntry>>>,
    /// TTL for org login -> org_id lookup cache used in `/events`.
    pub org_lookup_cache_ttl: Duration,
    /// Shared cache for org login -> org_id resolution.
    pub org_lookup_cache: Arc<Mutex<HashMap<String, OrgLookupCacheEntry>>>,
    /// TTL for repo full_name -> repo lookup cache used in `/events`.
    pub repo_lookup_cache_ttl: Duration,
    /// Shared cache for repo full_name -> repo resolution.
    pub repo_lookup_cache: Arc<Mutex<HashMap<String, RepoLookupCacheEntry>>>,
    /// Minimum interval between background repo upsert attempts for the same org/repo key.
    pub repo_upsert_min_interval: Duration,
    /// Debounce state for background repo upsert scheduling in `/events`.
    pub repo_upsert_last_attempt: Arc<Mutex<HashMap<String, Instant>>>,
    /// Minimum interval between cache invalidations under high ingest load.
    pub cache_invalidation_min_interval: Duration,
    /// Optional override for `/stats` cache invalidation debounce.
    pub stats_cache_invalidation_min_interval: Duration,
    /// Optional override for `/logs` cache invalidation debounce.
    pub logs_cache_invalidation_min_interval: Duration,
    /// Last wall-clock timestamp used to invalidate `/stats` cache.
    pub stats_cache_last_invalidation_ms: Arc<AtomicI64>,
    /// Single-flight guard to prevent stampede recomputation of `/stats` on cache miss.
    pub stats_cache_refresh_lock: Arc<AsyncMutex<()>>,
    /// In-memory short TTL cache for `/logs` payloads (keyed by scoped filter).
    pub logs_cache_ttl: Duration,
    /// Extra grace window to serve recently expired `/logs` cache when DB fails.
    pub logs_cache_stale_on_error: Duration,
    /// Optional hard deprecation flag for `/logs` offset pagination.
    /// When true, `/logs` rejects requests using `offset > 0` unless keyset cursor is used.
    pub logs_reject_offset_pagination: bool,
    /// Enables server-side outbox lease endpoint (`/outbox/lease`) for cross-host coordination.
    pub outbox_server_lease_enabled: bool,
    /// Default lease TTL used by `/outbox/lease` when client does not specify one.
    pub outbox_server_lease_ttl_ms: u64,
    /// In-memory telemetry for outbox lease traffic.
    pub outbox_lease_telemetry: Arc<Mutex<OutboxLeaseTelemetry>>,
    /// Cache keyed by effective filter + role scoping.
    pub logs_cache: Arc<Mutex<HashMap<String, LogsCacheEntry>>>,
    /// Last wall-clock timestamp used to invalidate `/logs` cache.
    pub logs_cache_last_invalidation_ms: Arc<AtomicI64>,
    /// Minimum interval between non-critical `client_sessions` upserts per client.
    pub client_session_upsert_min_interval: Duration,
    /// Per-client in-memory debounce state for session upserts.
    pub client_session_last_upsert: Arc<Mutex<HashMap<String, Instant>>>,
    /// Broadcast channel for SSE dashboard notifications.
    pub sse_tx: tokio::sync::broadcast::Sender<SseNotification>,
    /// Max concurrent SSE connections allowed server-wide.
    pub sse_max_connections: Arc<Semaphore>,
    /// Enables cross-instance SSE fan-out via PostgreSQL `NOTIFY`.
    pub sse_distributed_enabled: bool,
    /// PostgreSQL channel used for cross-instance SSE fan-out.
    pub sse_distributed_channel: String,
    /// Optional transport-level blocking scopes for `POST /policy/check`.
    /// Format source: `GITGOV_POLICY_CHECK_BLOCK_SCOPES=org:branch_glob`.
    pub policy_check_block_scopes: Vec<PolicyCheckBlockingScope>,
}

/// Lightweight notification sent via SSE to connected dashboard clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseNotification {
    /// New events were ingested via POST /events.
    NewEvents { count: u32 },
    /// Stats cache was invalidated (new data available).
    #[allow(dead_code)]
    StatsUpdated,
    /// Server heartbeat (keep-alive).
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedSseEnvelope {
    pub source_node: String,
    pub emitted_at_ms: i64,
    pub notification: SseNotification,
}

pub async fn fanout_sse_new_events(state: &Arc<AppState>, count: u32) {
    if count == 0 {
        return;
    }
    fanout_sse_notification(state, SseNotification::NewEvents { count }).await;
}

pub fn invalidate_dashboard_caches_for_sse(state: &AppState) {
    invalidate_stats_cache(state);
    invalidate_logs_cache(state);
}

pub async fn fanout_sse_notification(state: &Arc<AppState>, notification: SseNotification) {
    if matches!(notification, SseNotification::NewEvents { .. }) {
        invalidate_dashboard_caches_for_sse(state);
    }
    let _ = state.sse_tx.send(notification.clone());
    if !state.sse_distributed_enabled {
        return;
    }

    let envelope = DistributedSseEnvelope {
        source_node: state.worker_id.clone(),
        emitted_at_ms: chrono::Utc::now().timestamp_millis(),
        notification,
    };
    let payload = match serde_json::to_string(&envelope) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to serialize distributed SSE envelope");
            return;
        }
    };
    if let Err(e) = state
        .db
        .publish_sse_notification(&state.sse_distributed_channel, &payload)
        .await
    {
        tracing::warn!(
            error = %e,
            channel = %state.sse_distributed_channel,
            "Failed to publish distributed SSE notification"
        );
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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
