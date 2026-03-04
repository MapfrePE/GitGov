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
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;
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
    /// In-memory conversational runtime (context, todos, learning) keyed by user/org scope.
    pub conversational_runtime: Arc<Mutex<ConversationalRuntime>>,
    /// Max concurrent LLM chat calls handled by this server node.
    pub chat_llm_semaphore: Arc<Semaphore>,
    /// Max queue wait before rejecting a chat request as busy.
    pub chat_llm_queue_timeout_ms: u64,
    /// Max allowed duration for a single LLM chat call.
    pub chat_llm_timeout_ms: u64,
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

