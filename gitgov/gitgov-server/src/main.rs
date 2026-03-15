mod auth;
mod db;
mod handlers;
#[cfg(test)]
mod integration_tests;
mod models;
mod notifications;
mod openapi;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header::RETRY_AFTER, HeaderValue, Request, StatusCode},
    middleware,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
    Router,
};
use clap::Parser;
use dotenvy::dotenv;
use sha2::Digest;
use sqlx::postgres::PgListener;
use std::cmp::min;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicI64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::handlers::{AppState, PolicyCheckBlockingScope};

#[derive(Parser, Debug)]
#[command(name = "gitgov-server", about = "GitGov Control Plane")]
struct Args {
    #[arg(
        long,
        help = "Print bootstrap admin key to stdout (use for initial setup)"
    )]
    print_bootstrap_key: bool,
}

const JOB_WORKER_TTL_SECS: u64 = 300;
const JOB_POLL_INTERVAL_SECS: u64 = 5;
const JOB_ERROR_BACKOFF_SECS: u64 = 10;
const MIN_AUDIT_RETENTION_DAYS: i64 = 365 * 5;
const SIMULATE_RATE_LIMIT_INTERNAL_ERROR_ENV: &str = "GITGOV_SIMULATE_RATE_LIMIT_INTERNAL_ERROR";
const SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR_ENV: &str =
    "GITGOV_SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR";
const SSE_DISTRIBUTED_CHANNEL_DEFAULT: &str = "gitgov_sse_events";
const SSE_LISTENER_BACKOFF_START_SECS: u64 = 1;
const SSE_LISTENER_BACKOFF_MAX_SECS: u64 = 30;

#[derive(Debug)]
struct RateBucket {
    window_start: Instant,
    count: u32,
}

#[derive(Clone)]
struct InMemoryRateLimiter {
    name: &'static str,
    limit: u32,
    window: Duration,
    fail_open_on_lock_poison: bool,
    buckets: Arc<Mutex<HashMap<String, RateBucket>>>,
}

#[derive(Debug)]
struct RateLimitDecision {
    allowed: bool,
    retry_after_secs: u64,
    internal_error: bool,
}

#[derive(Clone)]
struct DistributedDbRateLimiter {
    name: &'static str,
    limit: u32,
    window: Duration,
    fail_open_on_db_error: bool,
    db: Arc<db::Database>,
    denied_until_cache: Arc<Mutex<HashMap<String, Instant>>>,
    denied_until_cache_max_entries: usize,
}

#[derive(Clone)]
enum RateLimiterState {
    InMemory(Arc<InMemoryRateLimiter>),
    DistributedDb(Arc<DistributedDbRateLimiter>),
}

impl InMemoryRateLimiter {
    fn new(
        name: &'static str,
        limit: u32,
        window: Duration,
        fail_open_on_lock_poison: bool,
    ) -> Self {
        Self {
            name,
            limit,
            window,
            fail_open_on_lock_poison,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn check(&self, key: &str) -> RateLimitDecision {
        if self.limit == 0 {
            return RateLimitDecision {
                allowed: true,
                retry_after_secs: 0,
                internal_error: false,
            };
        }

        if should_simulate_rate_limiter_internal_error(self.name) {
            if self.fail_open_on_lock_poison {
                tracing::warn!(
                    limiter = self.name,
                    "Simulating rate limiter internal error (debug failpoint, fail-open)"
                );
                return RateLimitDecision {
                    allowed: true,
                    retry_after_secs: 0,
                    internal_error: false,
                };
            }
            tracing::warn!(
                limiter = self.name,
                "Simulating rate limiter internal error (debug failpoint, fail-closed)"
            );
            return RateLimitDecision {
                allowed: false,
                retry_after_secs: 1,
                internal_error: true,
            };
        }

        let now = Instant::now();
        let mut buckets = match self.buckets.lock() {
            Ok(guard) => guard,
            Err(_) => {
                let mode = if self.fail_open_on_lock_poison {
                    "fail-open"
                } else {
                    "fail-closed"
                };
                tracing::warn!(limiter = self.name, mode, "Rate limiter lock poisoned");
                if self.fail_open_on_lock_poison {
                    return RateLimitDecision {
                        allowed: true,
                        retry_after_secs: 0,
                        internal_error: false,
                    };
                }
                return RateLimitDecision {
                    allowed: false,
                    retry_after_secs: 1,
                    internal_error: true,
                };
            }
        };

        // Opportunistic cleanup to prevent unbounded growth.
        if buckets.len() > 10_000 {
            let stale_after = self.window + self.window;
            buckets.retain(|_, bucket| now.duration_since(bucket.window_start) <= stale_after);
        }

        let bucket = buckets.entry(key.to_string()).or_insert(RateBucket {
            window_start: now,
            count: 0,
        });

        if now.duration_since(bucket.window_start) >= self.window {
            bucket.window_start = now;
            bucket.count = 0;
        }

        if bucket.count >= self.limit {
            let elapsed = now.duration_since(bucket.window_start);
            let retry_after = self.window.saturating_sub(elapsed).as_secs().max(1);
            return RateLimitDecision {
                allowed: false,
                retry_after_secs: retry_after,
                internal_error: false,
            };
        }

        bucket.count += 1;
        RateLimitDecision {
            allowed: true,
            retry_after_secs: 0,
            internal_error: false,
        }
    }
}

impl DistributedDbRateLimiter {
    fn new(
        name: &'static str,
        limit: u32,
        window: Duration,
        fail_open_on_db_error: bool,
        db: Arc<db::Database>,
    ) -> Self {
        Self {
            name,
            limit,
            window,
            fail_open_on_db_error,
            db,
            denied_until_cache: Arc::new(Mutex::new(HashMap::new())),
            denied_until_cache_max_entries: 16_384,
        }
    }

    async fn check(&self, key: &str) -> RateLimitDecision {
        if self.limit == 0 {
            return RateLimitDecision {
                allowed: true,
                retry_after_secs: 0,
                internal_error: false,
            };
        }

        if should_simulate_rate_limiter_internal_error(self.name) {
            if self.fail_open_on_db_error {
                tracing::warn!(
                    limiter = self.name,
                    "Simulating distributed rate limiter internal error (debug failpoint, fail-open)"
                );
                return RateLimitDecision {
                    allowed: true,
                    retry_after_secs: 0,
                    internal_error: false,
                };
            }
            tracing::warn!(
                limiter = self.name,
                "Simulating distributed rate limiter internal error (debug failpoint, fail-closed)"
            );
            return RateLimitDecision {
                allowed: false,
                retry_after_secs: 1,
                internal_error: true,
            };
        }

        if let Some(retry_after_secs) = get_cached_denied_retry_secs(&self.denied_until_cache, key)
        {
            return RateLimitDecision {
                allowed: false,
                retry_after_secs,
                internal_error: false,
            };
        }

        match self
            .db
            .check_distributed_rate_limit(self.name, key, self.limit, self.window)
            .await
        {
            Ok(result) => {
                if result.allowed {
                    clear_cached_denied_key(&self.denied_until_cache, key);
                } else {
                    put_cached_denied_retry_secs(
                        &self.denied_until_cache,
                        key,
                        result.retry_after_secs,
                        self.denied_until_cache_max_entries,
                    );
                }
                RateLimitDecision {
                    allowed: result.allowed,
                    retry_after_secs: result.retry_after_secs,
                    internal_error: false,
                }
            }
            Err(e) => {
                let mode = if self.fail_open_on_db_error {
                    "fail-open"
                } else {
                    "fail-closed"
                };
                tracing::warn!(
                    limiter = self.name,
                    mode,
                    error = %e,
                    "Distributed rate limiter DB check failed"
                );
                if self.fail_open_on_db_error {
                    RateLimitDecision {
                        allowed: true,
                        retry_after_secs: 0,
                        internal_error: false,
                    }
                } else {
                    RateLimitDecision {
                        allowed: false,
                        retry_after_secs: 1,
                        internal_error: true,
                    }
                }
            }
        }
    }
}

fn get_cached_denied_retry_secs(cache: &Mutex<HashMap<String, Instant>>, key: &str) -> Option<u64> {
    let now = Instant::now();
    let mut guard = cache.lock().ok()?;
    let denied_until = guard.get(key).copied()?;
    if denied_until <= now {
        guard.remove(key);
        return None;
    }
    Some(denied_until.duration_since(now).as_secs().max(1))
}

fn put_cached_denied_retry_secs(
    cache: &Mutex<HashMap<String, Instant>>,
    key: &str,
    retry_after_secs: u64,
    max_entries: usize,
) {
    if retry_after_secs == 0 {
        return;
    }
    let denied_until = Instant::now() + Duration::from_secs(retry_after_secs.max(1));
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    guard.insert(key.to_string(), denied_until);
    if guard.len() > max_entries {
        let now = Instant::now();
        guard.retain(|_, expires_at| *expires_at > now);
        if guard.len() > max_entries {
            let overflow = guard.len().saturating_sub(max_entries);
            let stale_keys = guard.keys().take(overflow).cloned().collect::<Vec<_>>();
            for stale_key in stale_keys {
                guard.remove(&stale_key);
            }
        }
    }
}

fn clear_cached_denied_key(cache: &Mutex<HashMap<String, Instant>>, key: &str) {
    if let Ok(mut guard) = cache.lock() {
        guard.remove(key);
    }
}

impl RateLimiterState {
    async fn check(&self, key: &str) -> RateLimitDecision {
        match self {
            Self::InMemory(limiter) => limiter.check(key),
            Self::DistributedDb(limiter) => limiter.check(key).await,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::InMemory(limiter) => limiter.name,
            Self::DistributedDb(limiter) => limiter.name,
        }
    }

    fn limit(&self) -> u32 {
        match self {
            Self::InMemory(limiter) => limiter.limit,
            Self::DistributedDb(limiter) => limiter.limit,
        }
    }
}

fn parse_u32_env(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

fn parse_usize_env(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn parse_bool_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn should_simulate_rate_limiter_internal_error(limiter_name: &str) -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    if !parse_bool_env(SIMULATE_RATE_LIMIT_INTERNAL_ERROR_ENV, false) {
        return false;
    }

    let raw_targets = std::env::var(SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR_ENV).unwrap_or_default();
    let trimmed = raw_targets.trim();
    if trimmed.is_empty() {
        return true;
    }

    raw_targets.split(',').any(|item| {
        let target = item.trim();
        !target.is_empty() && (target.eq_ignore_ascii_case("all") || target == limiter_name)
    })
}

fn parse_i64_env(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

fn parse_csv_env(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_policy_check_block_scopes_env(key: &str) -> Vec<PolicyCheckBlockingScope> {
    std::env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .filter_map(|entry| {
                    let mut parts = entry.splitn(2, ':');
                    let org_pattern = parts
                        .next()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(ToOwned::to_owned)?;
                    let branch_pattern = parts
                        .next()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .unwrap_or("*")
                        .to_string();
                    Some(PolicyCheckBlockingScope::new(org_pattern, branch_pattern))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_runtime_env() -> (String, bool, bool) {
    let runtime_env_explicit = std::env::var("GITGOV_ENV").is_ok();
    let default_env = if cfg!(debug_assertions) {
        "dev"
    } else {
        "prod"
    };
    let runtime_env = std::env::var("GITGOV_ENV")
        .unwrap_or_else(|_| default_env.to_string())
        .trim()
        .to_ascii_lowercase();
    let is_dev_env = matches!(
        runtime_env.as_str(),
        "dev" | "development" | "local" | "test"
    );
    (runtime_env, is_dev_env, runtime_env_explicit)
}

fn parse_cors_origins(input: &str) -> Vec<HeaderValue> {
    input
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect()
}

/// Build rate-limit key from the authenticated user identity.
/// Priority: authenticated identity (scoped by org when available) > auth token hash + IP.
/// This keeps authenticated rate limiting stable across IP changes and avoids
/// cross-tenant collisions when different orgs share the same login string.
fn rate_limit_key_from_request(req: &Request<Body>) -> String {
    // If auth middleware has already run, use the authenticated user identity.
    // For multi-tenant isolation, scope by org_id when available.
    if let Some(auth_user) = req.extensions().get::<auth::AuthUser>() {
        if let Some(org_id) = auth_user
            .org_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            return format!("org:{}:user:{}", org_id, auth_user.client_id);
        }
        return format!("user:{}", auth_user.client_id);
    }

    // Fallback for unauthenticated routes: IP + token hash (original behavior)
    let headers = req.headers();
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
                .map(str::trim)
                .filter(|v| !v.is_empty())
        })
        .unwrap_or("unknown");

    let auth_fingerprint = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(|auth| {
            let digest = sha2::Sha256::digest(auth.as_bytes());
            format!("{:x}", digest)[..12].to_string()
        })
        .unwrap_or_else(|| "noauth".to_string());

    format!("{}:{}", ip, auth_fingerprint)
}

async fn security_headers(req: Request<Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    // Modern browsers ignore X-XSS-Protection; value "0" disables legacy filter
    // to avoid introducing new vulnerabilities in older browsers.
    headers.insert("x-xss-protection", HeaderValue::from_static("0"));
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    response
}

/// Middleware that records per-request HTTP duration and status as Prometheus
/// histograms/counters.  The path is normalized to avoid high-cardinality label
/// explosion (UUIDs and numeric IDs are replaced with placeholders).
async fn request_metrics_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().to_string();
    let raw_path = req.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // Normalize path to avoid high-cardinality labels (UUIDs, numeric IDs).
    let path = normalize_metrics_path(&raw_path);

    metrics::histogram!("gitgov_http_request_duration_seconds",
        "method" => method, "path" => path, "status" => status
    )
    .record(duration);

    response
}

/// Replace UUID segments and numeric IDs with `{id}` to keep label cardinality bounded.
fn normalize_metrics_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for segment in path.split('/') {
        out.push('/');
        if segment.is_empty() {
            continue;
        }
        // UUID-like: 8-4-4-4-12 hex chars
        let is_uuid =
            segment.len() == 36 && segment.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        // Pure numeric
        let is_numeric = !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit());
        if is_uuid || is_numeric {
            out.push_str("{id}");
        } else {
            out.push_str(segment);
        }
    }
    if out.is_empty() {
        "/".to_string()
    } else {
        out
    }
}

async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiterState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let key = rate_limit_key_from_request(&req);
    let decision = limiter.check(&key).await;

    if decision.allowed {
        return next.run(req).await;
    }

    metrics::counter!("gitgov_rate_limited_total", "limiter" => limiter.name().to_string())
        .increment(1);

    if decision.internal_error {
        tracing::error!(
            limiter = limiter.name(),
            key = %key,
            "Rate limiter unavailable (internal error)"
        );
        let mut response = (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "Rate limiter temporarily unavailable",
                "code": "RATE_LIMITER_UNAVAILABLE",
                "retry_after_seconds": decision.retry_after_secs
            })),
        )
            .into_response();
        if let Ok(value) = HeaderValue::from_str(&decision.retry_after_secs.to_string()) {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        return response;
    }

    tracing::warn!(
        limiter = limiter.name(),
        key = %key,
        retry_after_secs = decision.retry_after_secs,
        "Request rate limited"
    );

    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        axum::Json(serde_json::json!({
            "error": "Too many requests",
            "code": "RATE_LIMITED",
            "retry_after_seconds": decision.retry_after_secs
        })),
    )
        .into_response();

    if let Ok(value) = HeaderValue::from_str(&decision.retry_after_secs.to_string()) {
        response.headers_mut().insert(RETRY_AFTER, value);
    }

    response
}

fn spawn_distributed_sse_listener(state: Arc<handlers::AppState>, database_url: String) {
    let channel = state.sse_distributed_channel.clone();
    let source_node = state.worker_id.clone();
    tokio::spawn(async move {
        let mut backoff_secs = SSE_LISTENER_BACKOFF_START_SECS;
        loop {
            let mut listener = match PgListener::connect(&database_url).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        channel = %channel,
                        backoff_secs,
                        "Distributed SSE listener failed to connect"
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    backoff_secs =
                        (backoff_secs.saturating_mul(2)).min(SSE_LISTENER_BACKOFF_MAX_SECS);
                    continue;
                }
            };

            if let Err(e) = listener.listen(&channel).await {
                tracing::warn!(
                    error = %e,
                    channel = %channel,
                    backoff_secs,
                    "Distributed SSE listener failed to subscribe"
                );
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs.saturating_mul(2)).min(SSE_LISTENER_BACKOFF_MAX_SECS);
                continue;
            }

            tracing::info!(channel = %channel, "Distributed SSE listener connected");
            backoff_secs = SSE_LISTENER_BACKOFF_START_SECS;
            loop {
                match listener.recv().await {
                    Ok(notification) => {
                        let payload = notification.payload();
                        let envelope =
                            match serde_json::from_str::<handlers::DistributedSseEnvelope>(payload)
                            {
                                Ok(parsed) => parsed,
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        channel = %channel,
                                        "Distributed SSE payload decode failed"
                                    );
                                    continue;
                                }
                            };
                        if envelope.source_node == source_node {
                            continue;
                        }
                        if matches!(
                            envelope.notification,
                            handlers::SseNotification::NewEvents { .. }
                        ) {
                            handlers::invalidate_dashboard_caches_for_sse(&state);
                        }
                        let _ = state.sse_tx.send(envelope.notification);
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            channel = %channel,
                            "Distributed SSE listener lost connection; reconnecting"
                        );
                        break;
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs.saturating_mul(2)).min(SSE_LISTENER_BACKOFF_MAX_SECS);
        }
    });
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Keep shared/legacy API model types linked under strict clippy settings.
    models::touch_contract_types();

    let args = Args::parse();
    let (runtime_env, is_dev_env, runtime_env_explicit) = parse_runtime_env();
    if !runtime_env_explicit {
        tracing::warn!(
            runtime_env = %runtime_env,
            "GITGOV_ENV not set explicitly; using compile-profile default"
        );
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gitgov_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Prometheus metrics recorder — must be installed before any metrics::* calls.
    let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus metrics recorder");
    tracing::info!("Prometheus metrics recorder installed");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let allow_insecure_jwt_fallback =
        parse_bool_env("GITGOV_ALLOW_INSECURE_JWT_FALLBACK", is_dev_env);
    let _jwt_secret = match std::env::var("GITGOV_JWT_SECRET") {
        Ok(secret) if !secret.trim().is_empty() => secret,
        _ if allow_insecure_jwt_fallback => {
            tracing::warn!(
                runtime_env = %runtime_env,
                "Using insecure JWT secret fallback; set GITGOV_JWT_SECRET for hardened environments"
            );
            "gitgov-secret-key-change-in-production".to_string()
        }
        _ => {
            tracing::error!(
                runtime_env = %runtime_env,
                "Missing GITGOV_JWT_SECRET in non-dev hardening mode"
            );
            std::process::exit(1);
        }
    };

    let github_webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if !is_dev_env && github_webhook_secret.is_none() {
        tracing::error!(
            runtime_env = %runtime_env,
            "Missing GITHUB_WEBHOOK_SECRET in non-dev hardening mode"
        );
        std::process::exit(1);
    }
    if is_dev_env && github_webhook_secret.is_none() {
        tracing::warn!(
            "GITHUB_WEBHOOK_SECRET is not configured; GitHub webhook signature validation is disabled"
        );
    }
    let github_personal_access_token = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN").ok();
    let jenkins_webhook_secret = std::env::var("JENKINS_WEBHOOK_SECRET").ok();
    let jira_webhook_secret = std::env::var("JIRA_WEBHOOK_SECRET").ok();

    let db = match db::Database::new(&database_url).await {
        Ok(db) => Arc::new(db),
        Err(e) => {
            tracing::error!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    tracing::info!("Connected to Supabase database");

    // Bootstrap: create first admin API key if none exist
    // Or use the key from GITGOV_API_KEY env if configured
    let should_print_key = args.print_bootstrap_key || atty::is(atty::Stream::Stderr);

    // Check if GITGOV_API_KEY is configured and keep it active as global founder admin key
    if let Ok(env_api_key) = std::env::var("GITGOV_API_KEY") {
        let key_hash = format!("{:x}", sha2::Sha256::digest(env_api_key.as_bytes()));

        match db.ensure_admin_api_key(&key_hash, "bootstrap-admin").await {
            Ok(_) => {
                tracing::info!("GITGOV_API_KEY ensured as active global Admin key");
                if should_print_key {
                    eprintln!();
                    eprintln!("╔════════════════════════════════════════════════════════════════╗");
                    eprintln!(
                        "║  GITGOV_API_KEY configured and ready                            ║"
                    );
                    eprintln!("╚════════════════════════════════════════════════════════════════╝");
                    eprintln!();
                }
            }
            Err(e) => {
                tracing::error!("Failed to ensure GITGOV_API_KEY: {}", e);
            }
        }
    } else {
        // No GITGOV_API_KEY in env, use normal bootstrap
        match db.bootstrap_admin_key().await {
            Ok(Some(api_key)) => {
                if should_print_key {
                    eprintln!();
                    eprintln!("╔════════════════════════════════════════════════════════════════╗");
                    eprintln!("║  BOOTSTRAP ADMIN KEY - SAVE NOW, WILL NOT BE SHOWN AGAIN       ║");
                    eprintln!("╠════════════════════════════════════════════════════════════════╣");
                    eprintln!("║  {}", api_key);
                    eprintln!("╚════════════════════════════════════════════════════════════════╝");
                    eprintln!();
                }
                tracing::info!(
                    print_key = should_print_key,
                    "Bootstrap admin key created. Use --print-bootstrap-key to display."
                );
            }
            Ok(None) => {
                tracing::info!("API keys already exist, skipping bootstrap");
            }
            Err(e) => {
                tracing::error!("Failed to bootstrap admin key: {}", e);
            }
        }
    }

    // Start job worker for background processing
    let db_for_worker = Arc::clone(&db);
    let worker_id = format!("worker-{}", std::process::id());
    let worker_id_clone = worker_id.clone();

    let _worker_handle = tokio::spawn(async move {
        tracing::info!(
            worker_id = %worker_id,
            ttl_secs = JOB_WORKER_TTL_SECS,
            poll_interval_secs = JOB_POLL_INTERVAL_SECS,
            "Job worker started"
        );

        let mut consecutive_errors = 0u32;

        loop {
            // Reset stale jobs periodically (safe, uses FOR UPDATE SKIP LOCKED)
            match db_for_worker.reset_stale_jobs().await {
                Ok(count) if count > 0 => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        stale_count = count,
                        "Reset stale jobs"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!(
                        worker_id = %worker_id,
                        error = %e,
                        "Failed to reset stale jobs"
                    );
                }
            }

            // Try to claim a job (atomic with FOR UPDATE SKIP LOCKED)
            match db_for_worker.claim_job(&worker_id).await {
                Ok(Some(job)) => {
                    consecutive_errors = 0;

                    tracing::info!(
                        worker_id = %worker_id,
                        job_id = %job.id,
                        org_id = %job.org_id,
                        job_type = %job.job_type,
                        attempt = job.attempts,
                        "Processing job"
                    );

                    // Execute job based on type
                    let result = match job.job_type.as_str() {
                        "detect_signals" => db_for_worker
                            .execute_detect_signals(&job.org_id)
                            .await
                            .map(|count| {
                                tracing::info!(
                                    worker_id = %worker_id,
                                    job_id = %job.id,
                                    org_id = %job.org_id,
                                    signals_created = count,
                                    "Signal detection completed"
                                );
                            }),
                        job_type => {
                            tracing::warn!(
                                worker_id = %worker_id,
                                job_id = %job.id,
                                job_type = %job_type,
                                "Unknown job type"
                            );
                            Err(db::DbError::DatabaseError(format!(
                                "Unknown job type: {}",
                                job_type
                            )))
                        }
                    };

                    // Handle result
                    match result {
                        Ok(()) => {
                            if let Err(e) = db_for_worker.complete_job(&job.id).await {
                                tracing::error!(
                                    worker_id = %worker_id,
                                    job_id = %job.id,
                                    error = %e,
                                    "Failed to mark job complete"
                                );
                            }
                        }
                        Err(e) => {
                            if let Err(err) = db_for_worker.fail_job(&job.id, &e.to_string()).await
                            {
                                tracing::error!(
                                    worker_id = %worker_id,
                                    job_id = %job.id,
                                    error = %err,
                                    "Failed to mark job failed"
                                );
                            }
                        }
                    }
                }
                Ok(None) => {
                    // No jobs available, wait before polling again
                    tokio::time::sleep(tokio::time::Duration::from_secs(JOB_POLL_INTERVAL_SECS))
                        .await;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    tracing::error!(
                        worker_id = %worker_id,
                        error = %e,
                        consecutive_errors = consecutive_errors,
                        "Failed to claim job"
                    );
                    // Exponential backoff on repeated errors
                    let backoff = JOB_ERROR_BACKOFF_SECS * (1 << consecutive_errors.min(5));
                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff)).await;
                }
            }
        }
    });

    let alert_webhook_url = std::env::var("GITGOV_ALERT_WEBHOOK_URL").ok();
    let mut drift_alert_webhook_urls = parse_csv_env("GITGOV_DRIFT_ALERT_WEBHOOK_URLS");
    if drift_alert_webhook_urls.is_empty() {
        if let Ok(single_url) = std::env::var("GITGOV_DRIFT_ALERT_WEBHOOK_URL") {
            let trimmed = single_url.trim();
            if !trimmed.is_empty() {
                drift_alert_webhook_urls.push(trimmed.to_string());
            }
        }
    }
    drift_alert_webhook_urls.sort();
    drift_alert_webhook_urls.dedup();
    let strict_actor_match = parse_bool_env("GITGOV_STRICT_ACTOR_MATCH", true);
    let reject_synthetic_logins = parse_bool_env("GITGOV_REJECT_SYNTHETIC_LOGINS", false);
    let llm_api_key = std::env::var("GEMINI_API_KEY").ok();
    let llm_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());
    let feature_request_webhook_url = std::env::var("FEATURE_REQUEST_WEBHOOK_URL").ok();
    let chat_llm_max_concurrency = parse_usize_env("GITGOV_CHAT_LLM_MAX_CONCURRENCY", 16);
    let chat_llm_queue_timeout_ms =
        parse_usize_env("GITGOV_CHAT_LLM_QUEUE_TIMEOUT_MS", 3000) as u64;
    let chat_llm_timeout_ms = parse_usize_env("GITGOV_CHAT_LLM_TIMEOUT_MS", 9000) as u64;
    let stats_cache_ttl_ms = parse_usize_env("GITGOV_STATS_CACHE_TTL_MS", 3000) as u64;
    let logs_cache_ttl_ms = parse_usize_env("GITGOV_LOGS_CACHE_TTL_MS", 800) as u64;
    let org_lookup_cache_ttl_ms = parse_usize_env("GITGOV_ORG_LOOKUP_CACHE_TTL_MS", 30_000) as u64;
    let repo_lookup_cache_ttl_ms =
        parse_usize_env("GITGOV_REPO_LOOKUP_CACHE_TTL_MS", 30_000) as u64;
    let repo_upsert_min_interval_ms =
        parse_usize_env("GITGOV_REPO_UPSERT_MIN_INTERVAL_MS", 30_000) as u64;
    let cache_invalidation_min_interval_ms =
        parse_usize_env("GITGOV_CACHE_INVALIDATION_MIN_INTERVAL_MS", 120) as u64;
    let stats_cache_invalidation_min_interval_ms = parse_usize_env(
        "GITGOV_STATS_CACHE_INVALIDATION_MIN_INTERVAL_MS",
        cache_invalidation_min_interval_ms as usize,
    ) as u64;
    let logs_cache_invalidation_min_interval_ms = parse_usize_env(
        "GITGOV_LOGS_CACHE_INVALIDATION_MIN_INTERVAL_MS",
        cache_invalidation_min_interval_ms as usize,
    ) as u64;
    let logs_cache_stale_on_error_ms =
        parse_usize_env("GITGOV_LOGS_CACHE_STALE_ON_ERROR_MS", 5000) as u64;
    let client_session_upsert_min_interval_ms =
        parse_usize_env("GITGOV_CLIENT_SESSION_UPSERT_MIN_INTERVAL_MS", 15_000) as u64;
    let logs_reject_offset_pagination =
        parse_bool_env("GITGOV_LOGS_REJECT_OFFSET_PAGINATION", false);
    let outbox_server_lease_requested = parse_bool_env("GITGOV_OUTBOX_SERVER_LEASE_ENABLED", false);
    let outbox_server_lease_ttl_ms =
        parse_usize_env("GITGOV_OUTBOX_SERVER_LEASE_TTL_MS", 2_000).clamp(1_000, 60_000) as u64;
    let sse_distributed_enabled = parse_bool_env("GITGOV_SSE_DISTRIBUTED_ENABLED", false);
    let sse_distributed_channel = std::env::var("GITGOV_SSE_DISTRIBUTED_CHANNEL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| SSE_DISTRIBUTED_CHANNEL_DEFAULT.to_string());
    let events_max_batch = parse_usize_env("GITGOV_EVENTS_MAX_BATCH", 1000);
    let policy_check_block_scopes =
        parse_policy_check_block_scopes_env("GITGOV_POLICY_CHECK_BLOCK_SCOPES");
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client for notifications");
    let outbox_server_lease_enabled = if outbox_server_lease_requested {
        match db.ensure_outbox_lease_storage().await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to initialize outbox lease storage; disabling server lease endpoint"
                );
                false
            }
        }
    } else {
        false
    };

    let state = Arc::new(AppState {
        db: Arc::clone(&db),
        github_webhook_secret,
        github_personal_access_token,
        jenkins_webhook_secret,
        jira_webhook_secret,
        start_time: Instant::now(),
        worker_id: worker_id_clone.clone(),
        http_client,
        alert_webhook_url,
        drift_alert_webhook_urls,
        strict_actor_match,
        reject_synthetic_logins,
        events_max_batch,
        llm_api_key,
        llm_model,
        feature_request_webhook_url,
        conversational_runtime: Arc::new(std::sync::Mutex::new(
            handlers::ConversationalRuntime::default(),
        )),
        chat_llm_semaphore: Arc::new(Semaphore::new(chat_llm_max_concurrency)),
        chat_llm_queue_timeout_ms,
        chat_llm_timeout_ms,
        stats_cache_ttl: Duration::from_millis(stats_cache_ttl_ms),
        stats_cache: Arc::new(Mutex::new(HashMap::new())),
        org_lookup_cache_ttl: Duration::from_millis(org_lookup_cache_ttl_ms),
        org_lookup_cache: Arc::new(Mutex::new(HashMap::new())),
        repo_lookup_cache_ttl: Duration::from_millis(repo_lookup_cache_ttl_ms),
        repo_lookup_cache: Arc::new(Mutex::new(HashMap::new())),
        repo_upsert_min_interval: Duration::from_millis(repo_upsert_min_interval_ms),
        repo_upsert_last_attempt: Arc::new(Mutex::new(HashMap::new())),
        cache_invalidation_min_interval: Duration::from_millis(cache_invalidation_min_interval_ms),
        stats_cache_invalidation_min_interval: Duration::from_millis(
            stats_cache_invalidation_min_interval_ms,
        ),
        logs_cache_invalidation_min_interval: Duration::from_millis(
            logs_cache_invalidation_min_interval_ms,
        ),
        stats_cache_last_invalidation_ms: Arc::new(AtomicI64::new(0)),
        stats_cache_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        logs_cache_ttl: Duration::from_millis(logs_cache_ttl_ms),
        logs_cache_stale_on_error: Duration::from_millis(logs_cache_stale_on_error_ms),
        logs_reject_offset_pagination,
        outbox_server_lease_enabled,
        outbox_server_lease_ttl_ms,
        outbox_lease_telemetry: Arc::new(Mutex::new(handlers::OutboxLeaseTelemetry::default())),
        logs_cache: Arc::new(Mutex::new(HashMap::new())),
        logs_cache_last_invalidation_ms: Arc::new(AtomicI64::new(0)),
        client_session_upsert_min_interval: Duration::from_millis(
            client_session_upsert_min_interval_ms,
        ),
        client_session_last_upsert: Arc::new(Mutex::new(HashMap::new())),
        sse_tx: tokio::sync::broadcast::channel::<handlers::SseNotification>(64).0,
        sse_max_connections: Arc::new(Semaphore::new(
            parse_u32_env("GITGOV_SSE_MAX_CONNECTIONS", 50) as usize,
        )),
        sse_distributed_enabled,
        sse_distributed_channel: sse_distributed_channel.clone(),
        policy_check_block_scopes: policy_check_block_scopes.clone(),
    });

    if sse_distributed_enabled {
        spawn_distributed_sse_listener(Arc::clone(&state), database_url.clone());
    }

    if policy_check_block_scopes.is_empty() {
        tracing::info!("Policy check transport mode: advisory-only (default)");
    } else {
        let scopes = policy_check_block_scopes
            .iter()
            .map(|scope| format!("{}:{}", scope.org_pattern, scope.branch_pattern))
            .collect::<Vec<_>>()
            .join(", ");
        tracing::info!(
            scopes = %scopes,
            "Policy check transport mode: blocking for matching scopes"
        );
    }

    // Keep utility APIs exercised in non-test builds so strict linting does not
    // regress when these entry points are consumed by other binaries/tools.
    let _ = db::Database::get_github_events;
    let _ = db::Database::get_client_events;
    let _ = db::Database::reset_stale_jobs_safe;
    let _ = models::SignalType::as_str;
    let _ = models::SignalType::from_str;
    let _ = models::ConfidenceLevel::as_str;
    let _ = models::ConfidenceLevel::from_str;
    let _ = models::SignalStatus::as_str;
    let _ = models::SignalStatus::from_str;
    let _ = models::expand_login_aliases;

    let worker_id_for_log = worker_id_clone;

    // Audit trail retention policy (append-only tables are NOT deleted here).
    // Compliance guard: minimum 5 years for audit data.
    let configured_audit_retention_days =
        parse_i64_env("AUDIT_RETENTION_DAYS", MIN_AUDIT_RETENTION_DAYS);
    let effective_audit_retention_days =
        if configured_audit_retention_days < MIN_AUDIT_RETENTION_DAYS {
            tracing::warn!(
                configured = configured_audit_retention_days,
                min_days = MIN_AUDIT_RETENTION_DAYS,
                "AUDIT_RETENTION_DAYS below compliance minimum; clamping to min"
            );
            MIN_AUDIT_RETENTION_DAYS
        } else {
            configured_audit_retention_days
        };
    tracing::info!(
        audit_retention_days = effective_audit_retention_days,
        "Audit retention policy loaded (append-only audit tables)"
    );

    // TTL cleanup task — prunes stale client_sessions rows only.
    // Backward compatibility: DATA_RETENTION_DAYS still works as fallback.
    let db_ttl = Arc::clone(&db);
    let client_session_retention_days = parse_i64_env(
        "CLIENT_SESSION_RETENTION_DAYS",
        parse_i64_env("DATA_RETENTION_DAYS", 365),
    );
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86_400)); // 24 h
        interval.tick().await; // skip first tick (fires immediately)
        loop {
            interval.tick().await;
            match db_ttl
                .delete_old_events(client_session_retention_days)
                .await
            {
                Ok(count) => tracing::info!(
                    deleted = count,
                    retention_days = client_session_retention_days,
                    "TTL cleanup: deleted stale client sessions"
                ),
                Err(e) => tracing::warn!(error = %e, "TTL cleanup failed"),
            }
        }
    });

    let admin_rate_limit_per_min = parse_u32_env("GITGOV_RATE_LIMIT_ADMIN_PER_MIN", 60);
    let logs_rate_limit_per_min =
        parse_u32_env("GITGOV_RATE_LIMIT_LOGS_PER_MIN", admin_rate_limit_per_min);
    let stats_rate_limit_per_min =
        parse_u32_env("GITGOV_RATE_LIMIT_STATS_PER_MIN", admin_rate_limit_per_min);
    let distributed_rate_limit_requested =
        parse_bool_env("GITGOV_RATE_LIMIT_DISTRIBUTED_DB", false);
    let distributed_rate_limit_enabled = if distributed_rate_limit_requested {
        match db.ensure_rate_limit_storage().await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to initialize distributed rate limiter storage; falling back to in-memory limiter"
                );
                false
            }
        }
    } else {
        false
    };
    if distributed_rate_limit_enabled {
        let db_rate_limit_prune = Arc::clone(&db);
        let prune_interval_secs =
            parse_u32_env("GITGOV_RATE_LIMIT_DISTRIBUTED_PRUNE_INTERVAL_SECS", 300).max(30) as u64;
        let retention_secs =
            parse_u32_env("GITGOV_RATE_LIMIT_DISTRIBUTED_RETENTION_SECS", 3600).max(120) as u64;
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(prune_interval_secs));
            interval.tick().await;
            loop {
                interval.tick().await;
                match db_rate_limit_prune
                    .prune_rate_limit_counters(Duration::from_secs(retention_secs))
                    .await
                {
                    Ok(count) if count > 0 => {
                        tracing::debug!(
                            pruned = count,
                            retention_secs,
                            "Pruned distributed rate limiter counters"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed pruning distributed rate limiter counters"
                        );
                    }
                }
            }
        });
    }

    let make_rate_limiter = |name: &'static str, limit: u32, fail_open_on_internal_error: bool| {
        if distributed_rate_limit_enabled {
            Arc::new(RateLimiterState::DistributedDb(Arc::new(
                DistributedDbRateLimiter::new(
                    name,
                    limit,
                    Duration::from_secs(60),
                    fail_open_on_internal_error,
                    Arc::clone(&db),
                ),
            )))
        } else {
            Arc::new(RateLimiterState::InMemory(Arc::new(
                InMemoryRateLimiter::new(
                    name,
                    limit,
                    Duration::from_secs(60),
                    fail_open_on_internal_error,
                ),
            )))
        }
    };

    let events_rate_limit = make_rate_limiter(
        "events",
        parse_u32_env("GITGOV_RATE_LIMIT_EVENTS_PER_MIN", 240),
        true,
    );
    let audit_stream_rate_limit = make_rate_limiter(
        "audit_stream",
        parse_u32_env("GITGOV_RATE_LIMIT_AUDIT_STREAM_PER_MIN", 60),
        true,
    );
    let jenkins_rate_limit = make_rate_limiter(
        "jenkins_integrations",
        parse_u32_env("GITGOV_RATE_LIMIT_JENKINS_PER_MIN", 120),
        true,
    );
    let jira_rate_limit = make_rate_limiter(
        "jira_integrations",
        parse_u32_env("GITGOV_RATE_LIMIT_JIRA_PER_MIN", 120),
        true,
    );
    let admin_rate_limit = make_rate_limiter("admin_endpoints", admin_rate_limit_per_min, false);
    let logs_rate_limit = make_rate_limiter("logs_endpoints", logs_rate_limit_per_min, true);
    let stats_rate_limit = make_rate_limiter("stats_endpoints", stats_rate_limit_per_min, false);
    let chat_rate_limit = make_rate_limiter(
        "chat_endpoints",
        parse_u32_env("GITGOV_RATE_LIMIT_CHAT_PER_MIN", 40),
        true,
    );
    let events_body_limit_bytes = parse_usize_env("GITGOV_EVENTS_MAX_BODY_BYTES", 2 * 1024 * 1024);
    let jenkins_body_limit_bytes = parse_usize_env("GITGOV_JENKINS_MAX_BODY_BYTES", 256 * 1024);
    let jira_body_limit_bytes = parse_usize_env("GITGOV_JIRA_MAX_BODY_BYTES", 512 * 1024);
    let cors_allow_any = parse_bool_env("GITGOV_CORS_ALLOW_ANY", is_dev_env);
    let configured_cors_origins = std::env::var("GITGOV_CORS_ALLOW_ORIGINS").unwrap_or_default();
    let mut cors_origin_values = parse_cors_origins(&configured_cors_origins);
    if cors_origin_values.is_empty() && is_dev_env {
        cors_origin_values =
            parse_cors_origins("http://127.0.0.1:1420,http://localhost:1420,tauri://localhost");
    }
    if !cors_allow_any && cors_origin_values.is_empty() {
        tracing::error!(
            runtime_env = %runtime_env,
            "CORS strict mode enabled but no allowed origins configured; set GITGOV_CORS_ALLOW_ORIGINS"
        );
        std::process::exit(1);
    }
    let cors_origin_count = cors_origin_values.len();
    let cors_layer = if cors_allow_any {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(cors_origin_values))
            .allow_methods(Any)
            .allow_headers(Any)
    };

    tracing::info!(
        runtime_env = %runtime_env,
        rate_limit_mode = if distributed_rate_limit_enabled { "distributed_db" } else { "in_memory" },
        cors_allow_any,
        cors_origin_count = min(cors_origin_count, 50),
        events_per_min = events_rate_limit.limit(),
        audit_stream_per_min = audit_stream_rate_limit.limit(),
        jenkins_per_min = jenkins_rate_limit.limit(),
        jira_per_min = jira_rate_limit.limit(),
        events_body_limit_bytes,
        events_max_batch,
        jenkins_body_limit_bytes,
        jira_body_limit_bytes,
        admin_per_min = admin_rate_limit.limit(),
        logs_per_min = logs_rate_limit.limit(),
        stats_per_min = stats_rate_limit.limit(),
        chat_per_min = chat_rate_limit.limit(),
        chat_llm_max_concurrency,
        chat_llm_queue_timeout_ms,
        chat_llm_timeout_ms,
        stats_cache_ttl_ms,
        logs_cache_ttl_ms,
        org_lookup_cache_ttl_ms,
        repo_lookup_cache_ttl_ms,
        repo_upsert_min_interval_ms,
        cache_invalidation_min_interval_ms,
        stats_cache_invalidation_min_interval_ms,
        logs_cache_invalidation_min_interval_ms,
        logs_cache_stale_on_error_ms,
        client_session_upsert_min_interval_ms,
        logs_reject_offset_pagination,
        outbox_server_lease_enabled,
        outbox_server_lease_ttl_ms,
        sse_distributed_enabled,
        sse_distributed_channel = %sse_distributed_channel,
        "Rate limiting enabled for ingestion and control plane endpoints"
    );

    let auth_routes = Router::new()
        .route(
            "/logs",
            get(handlers::get_logs).layer(middleware::from_fn_with_state(
                Arc::clone(&logs_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/sse",
            get(handlers::sse_stream).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/stats",
            get(handlers::get_stats).layer(middleware::from_fn_with_state(
                Arc::clone(&stats_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/stats/daily",
            get(handlers::get_daily_activity).layer(middleware::from_fn_with_state(
                Arc::clone(&stats_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/dashboard",
            get(handlers::get_dashboard).layer(middleware::from_fn_with_state(
                Arc::clone(&stats_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/team/overview",
            get(handlers::get_team_overview).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/team/repos",
            get(handlers::get_team_repos).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/integrations/jenkins",
            post(handlers::ingest_jenkins_pipeline_event)
                .layer(DefaultBodyLimit::max(jenkins_body_limit_bytes))
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&jenkins_rate_limit),
                    rate_limit_middleware,
                )),
        )
        .route(
            "/integrations/jenkins/status",
            get(handlers::get_jenkins_integration_status),
        )
        .route(
            "/integrations/jenkins/correlations",
            get(handlers::get_jenkins_commit_correlations),
        )
        .route(
            "/integrations/correlations/v2",
            get(handlers::get_correlation_v2),
        )
        .route(
            "/integrations/jira",
            post(handlers::ingest_jira_webhook)
                .layer(DefaultBodyLimit::max(jira_body_limit_bytes))
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&jira_rate_limit),
                    rate_limit_middleware,
                )),
        )
        .route(
            "/integrations/jira/status",
            get(handlers::get_jira_integration_status),
        )
        .route(
            "/integrations/jira/tickets/{ticket_id}",
            get(handlers::get_jira_ticket_detail),
        )
        .route(
            "/integrations/jira/correlate",
            post(handlers::correlate_jira_tickets),
        )
        .route(
            "/integrations/jira/ticket-coverage",
            get(handlers::get_jira_ticket_coverage),
        )
        .route(
            "/compliance/{org_name}",
            get(handlers::get_compliance_dashboard),
        )
        .route("/signals", get(handlers::get_signals))
        .route("/signals/{signal_id}", post(handlers::update_signal))
        .route(
            "/signals/{signal_id}/confirm",
            post(handlers::confirm_signal),
        )
        .route(
            "/signals/detect/{org_name}",
            post(handlers::trigger_detection),
        )
        .route(
            "/violations/{violation_id}/decisions",
            get(handlers::get_violation_decisions),
        )
        .route(
            "/violations/{violation_id}/decisions",
            post(handlers::add_violation_decision),
        )
        .route("/policy/{repo_name}", get(handlers::get_policy))
        .route("/policy/check", post(handlers::policy_check))
        .route(
            "/policy/{repo_name}/history",
            get(handlers::get_policy_history),
        )
        .route(
            "/policy/{repo_name}/override",
            put(handlers::override_policy),
        )
        .route(
            "/policy/{repo_name}/requests",
            post(handlers::create_policy_change_request).get(handlers::list_policy_change_requests),
        )
        .route(
            "/policy/requests/{request_id}/approve",
            post(handlers::approve_policy_change_request),
        )
        .route(
            "/policy/requests/{request_id}/reject",
            post(handlers::reject_policy_change_request),
        )
        .route("/export", post(handlers::export_events))
        .route("/exports", get(handlers::list_exports))
        .route("/me", get(handlers::get_me))
        .route("/orgs", post(handlers::create_org))
        .route(
            "/org-users",
            get(handlers::list_org_users).post(handlers::create_org_user),
        )
        .route(
            "/org-users/{id}/status",
            patch(handlers::update_org_user_status),
        )
        .route(
            "/org-users/{id}/api-key",
            post(handlers::create_api_key_for_org_user),
        )
        .route(
            "/org-invitations",
            get(handlers::list_org_invitations).post(handlers::create_org_invitation),
        )
        .route(
            "/org-invitations/{id}/resend",
            post(handlers::resend_org_invitation),
        )
        .route(
            "/org-invitations/{id}/revoke",
            post(handlers::revoke_org_invitation),
        )
        .route(
            "/api-keys",
            get(handlers::list_api_keys)
                .post(handlers::create_api_key)
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&admin_rate_limit),
                    rate_limit_middleware,
                )),
        )
        .route(
            "/api-keys/{id}/revoke",
            post(handlers::revoke_api_key).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/events",
            post(handlers::ingest_client_events)
                .layer(DefaultBodyLimit::max(events_body_limit_bytes))
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&events_rate_limit),
                    rate_limit_middleware,
                )),
        )
        .route(
            "/outbox/lease",
            post(handlers::acquire_outbox_flush_lease).layer(middleware::from_fn_with_state(
                Arc::clone(&events_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/outbox/lease/metrics",
            get(handlers::get_outbox_lease_metrics).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/audit-stream/github",
            post(handlers::ingest_audit_stream).layer(middleware::from_fn_with_state(
                Arc::clone(&audit_stream_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route("/governance-events", get(handlers::get_governance_events))
        .route(
            "/pr-merges",
            get(handlers::list_pr_merges).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/admin-audit-log",
            get(handlers::list_admin_audit_log).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/jobs/metrics",
            get(handlers::get_job_metrics).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/jobs/dead",
            get(handlers::get_dead_jobs).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/jobs/{job_id}/retry",
            post(handlers::retry_dead_job).layer(middleware::from_fn_with_state(
                Arc::clone(&admin_rate_limit),
                rate_limit_middleware,
            )),
        )
        // GDPR — T2
        .route("/users/{login}/erase", post(handlers::erase_user))
        .route("/users/{login}/export", get(handlers::export_user))
        // Client sessions — T3.A
        .route("/clients", get(handlers::get_clients))
        // Identity aliases — T3.B
        .route(
            "/identities/aliases",
            post(handlers::create_identity_alias).get(handlers::list_identity_aliases),
        )
        // Conversational Chat (MVP)
        .route(
            "/chat/ask",
            post(handlers::chat_ask).layer(middleware::from_fn_with_state(
                Arc::clone(&chat_rate_limit),
                rate_limit_middleware,
            )),
        )
        .route(
            "/feature-requests",
            post(handlers::create_feature_request_handler),
        )
        // CLI Command Audit
        .route(
            "/cli/commands",
            post(handlers::ingest_cli_command).get(handlers::list_cli_commands),
        )
        .route(
            "/policy/drift-events",
            post(handlers::ingest_policy_drift_event).get(handlers::list_policy_drift_events),
        )
        .layer(middleware::from_fn_with_state(
            Arc::clone(&db),
            auth::auth_middleware,
        ));

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/health/detailed", get(handlers::detailed_health))
        .route("/webhooks/github", post(handlers::handle_github_webhook))
        .route("/metrics", get(handlers::prometheus_metrics))
        .route(
            "/org-invitations/preview/{token}",
            get(handlers::preview_org_invitation),
        )
        .route(
            "/org-invitations/accept",
            post(handlers::accept_org_invitation),
        )
        .merge(auth_routes)
        .merge(
            utoipa_swagger_ui::SwaggerUi::new("/api-docs")
                .url("/api-docs/openapi.json", openapi::build_openapi_spec()),
        )
        .layer(axum::Extension(prometheus_handle))
        .layer(cors_layer)
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn(request_metrics_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::clone(&state));

    let addr: SocketAddr = std::env::var("GITGOV_SERVER_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
        .parse()
        .expect("Invalid server address");

    tracing::info!("GitGov Control Plane starting on {}", addr);
    tracing::info!("Using Supabase PostgreSQL database");
    tracing::info!("");
    tracing::info!(
        configured = state.alert_webhook_url.is_some(),
        "Generic alert webhook configured"
    );
    tracing::info!(
        drift_webhook_targets = state.drift_alert_webhook_urls.len(),
        "Dedicated drift alert webhook targets configured"
    );
    tracing::info!("");
    tracing::info!("Endpoints:");
    tracing::info!("  GET  /health                    - Health check (public)");
    tracing::info!("  GET  /health/detailed           - Detailed health (public)");
    tracing::info!("  POST /webhooks/github           - GitHub webhook (HMAC auth)");
    tracing::info!("  GET  /metrics                   - Prometheus metrics (public)");
    tracing::info!("  GET  /api-docs                  - Swagger UI (public)");
    tracing::info!("  --- Authenticated endpoints ---");
    tracing::info!("  POST /events                    - Client events (auth)");
    tracing::info!("  POST /outbox/lease              - Outbox coordination lease (auth, opt-in)");
    tracing::info!("  GET  /outbox/lease/metrics      - Outbox lease telemetry (admin)");
    tracing::info!("  GET  /logs                      - Query events (auth, dev: own only)");
    tracing::info!("  GET  /pr-merges                 - PR merge evidence (admin)");
    tracing::info!("  GET  /stats                     - Statistics (admin)");
    tracing::info!("  GET  /stats/daily               - Daily commits/pushes series (admin)");
    tracing::info!("  GET  /dashboard                 - Dashboard (admin)");
    tracing::info!("  GET  /team/overview             - Team overview by developer (admin)");
    tracing::info!("  GET  /team/repos                - Team overview by repository (admin)");
    tracing::info!("  POST /integrations/jenkins      - Jenkins pipeline ingest (admin)");
    tracing::info!("  GET  /integrations/jenkins/status - Jenkins integration health (admin)");
    tracing::info!(
        "  GET  /integrations/jenkins/correlations - Commit->pipeline correlations (admin)"
    );
    tracing::info!("  GET  /integrations/correlations/v2 - Ticket->commit->pipeline view (admin)");
    tracing::info!("  POST /integrations/jira         - Jira webhook ingest (admin)");
    tracing::info!("  GET  /integrations/jira/status  - Jira integration health (admin)");
    tracing::info!("  GET  /integrations/jira/tickets/:ticket_id - Jira ticket detail (admin)");
    tracing::info!(
        "  POST /integrations/jira/correlate - Build commit<->ticket correlations (admin)"
    );
    tracing::info!("  GET  /integrations/jira/ticket-coverage - Ticket coverage metrics (admin)");
    tracing::info!("  GET  /compliance/:org           - Compliance (admin)");
    tracing::info!("  GET  /signals                   - Signals (auth)");
    tracing::info!("  POST /signals/:id               - Update signal (auth)");
    tracing::info!("  POST /signals/:id/confirm       - Confirm signal (admin)");
    tracing::info!("  POST /signals/detect/:org       - Trigger detection (admin)");
    tracing::info!("  --- Violation Decisions ---");
    tracing::info!("  GET  /violations/:id/decisions  - Get decision history (auth)");
    tracing::info!("  POST /violations/:id/decisions  - Add decision (admin)");
    tracing::info!("  GET  /policy/:repo              - Get policy (auth)");
    tracing::info!("  POST /policy/check              - Policy check (advisory + optional 409 block by scope, admin)");
    tracing::info!("  PUT  /policy/:repo/override     - Override policy (admin)");
    tracing::info!("  GET  /policy/:repo/history      - History (auth)");
    tracing::info!("  POST /policy/:repo/requests     - Create policy change request (auth)");
    tracing::info!("  GET  /policy/:repo/requests     - List policy change requests (auth)");
    tracing::info!("  POST /policy/requests/:id/approve - Approve policy change request (admin)");
    tracing::info!("  POST /policy/requests/:id/reject  - Reject policy change request (admin)");
    tracing::info!("  POST /policy/drift-events       - Ingest drift audit event (auth)");
    tracing::info!("  GET  /policy/drift-events       - List drift audit events (auth)");
    tracing::info!("  POST /export                    - Export (auth)");
    tracing::info!("  POST /orgs                      - Create/upsert org (admin)");
    tracing::info!("  POST /org-users                 - Create/update org user (admin)");
    tracing::info!("  GET  /org-users                 - List org users (admin)");
    tracing::info!("  PATCH /org-users/:id/status     - Activate/disable org user (admin)");
    tracing::info!("  POST /org-users/:id/api-key     - Issue API key for org user (admin)");
    tracing::info!("  POST /org-invitations           - Create org invitation (admin)");
    tracing::info!("  GET  /org-invitations           - List org invitations (admin)");
    tracing::info!("  POST /org-invitations/:id/resend - Regenerate invite token (admin)");
    tracing::info!("  POST /org-invitations/:id/revoke - Revoke invite (admin)");
    tracing::info!("  GET  /org-invitations/preview/:token - Preview invite (public)");
    tracing::info!("  POST /org-invitations/accept    - Accept invite and issue key (public)");
    tracing::info!("  POST /api-keys                  - Create API key (admin)");
    tracing::info!("  POST /audit-stream/github       - GitHub audit log stream (admin)");
    tracing::info!(
        "  (opt) JENKINS_WEBHOOK_SECRET    - Extra shared secret header x-gitgov-jenkins-secret"
    );
    tracing::info!(
        "  (opt) JIRA_WEBHOOK_SECRET       - Extra shared secret header x-gitgov-jira-secret"
    );
    tracing::info!(
        "  (opt) GITGOV_ALERT_WEBHOOK_URL  - Generic alert webhook (Slack/Discord/Teams)"
    );
    tracing::info!(
        "  (opt) GITGOV_DRIFT_ALERT_WEBHOOK_URLS - Dedicated drift alert webhooks (CSV)"
    );
    tracing::info!("  GET  /governance-events         - Query governance events (auth)");
    tracing::info!("  --- Job Queue Management ---");
    tracing::info!("  GET  /jobs/metrics              - Job queue metrics (admin)");
    tracing::info!("  GET  /jobs/dead                 - List dead jobs (admin)");
    tracing::info!("  POST /jobs/:id/retry            - Retry dead job (admin)");
    tracing::info!("");
    tracing::info!(
        worker_id = %worker_id_for_log,
        ttl_secs = JOB_WORKER_TTL_SECS,
        poll_interval_secs = JOB_POLL_INTERVAL_SECS,
        "Job worker configuration"
    );

    // TLS support: if GITGOV_TLS_CERT and GITGOV_TLS_KEY are set, serve HTTPS.
    let tls_cert = std::env::var("GITGOV_TLS_CERT").ok();
    let tls_key = std::env::var("GITGOV_TLS_KEY").ok();

    match (tls_cert, tls_key) {
        (Some(cert_path), Some(key_path)) => {
            let tls_config =
                axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!(
                            cert_path = %cert_path,
                            key_path = %key_path,
                            error = %e,
                            "Failed to load TLS certificate/key"
                        );
                        std::process::exit(1);
                    });

            tracing::info!(
                addr = %addr,
                cert = %cert_path,
                "Starting HTTPS server with TLS"
            );

            axum_server::bind_rustls(addr, tls_config)
                .serve(app.into_make_service())
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("HTTPS server error: {}", e);
                });
        }
        (Some(_), None) | (None, Some(_)) => {
            tracing::error!(
                "Both GITGOV_TLS_CERT and GITGOV_TLS_KEY must be set for HTTPS. Only one was provided."
            );
            std::process::exit(1);
        }
        (None, None) => {
            if !is_dev_env {
                tracing::warn!(
                    "HTTPS is NOT enabled. Set GITGOV_TLS_CERT and GITGOV_TLS_KEY for production TLS. \
                     Use a reverse proxy (nginx/caddy) if terminating TLS externally."
                );
            }

            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to bind to address {}: {}", addr, e);
                    std::process::exit(1);
                });

            axum::serve(listener, app).await.unwrap_or_else(|e| {
                tracing::error!("Server error: {}", e);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tower::ServiceExt;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env_var(key: &str, value: &str) {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var(key);
        }
    }

    fn set_or_clear_env(key: &str, value: Option<&str>) {
        match value {
            Some(v) => set_env_var(key, v),
            None => remove_env_var(key),
        }
    }

    struct EnvGuard {
        simulate_internal_error: Option<String>,
        simulate_internal_error_for: Option<String>,
    }

    impl EnvGuard {
        fn apply(simulate_internal_error: &str, simulate_internal_error_for: Option<&str>) -> Self {
            let guard = Self {
                simulate_internal_error: std::env::var(SIMULATE_RATE_LIMIT_INTERNAL_ERROR_ENV).ok(),
                simulate_internal_error_for: std::env::var(
                    SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR_ENV,
                )
                .ok(),
            };
            set_env_var(
                SIMULATE_RATE_LIMIT_INTERNAL_ERROR_ENV,
                simulate_internal_error,
            );
            match simulate_internal_error_for {
                Some(value) => set_env_var(SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR_ENV, value),
                None => remove_env_var(SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR_ENV),
            }
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            set_or_clear_env(
                SIMULATE_RATE_LIMIT_INTERNAL_ERROR_ENV,
                self.simulate_internal_error.as_deref(),
            );
            set_or_clear_env(
                SIMULATE_RATE_LIMIT_INTERNAL_ERROR_FOR_ENV,
                self.simulate_internal_error_for.as_deref(),
            );
        }
    }

    fn poison_limiter_lock(limiter: &InMemoryRateLimiter) {
        let buckets = Arc::clone(&limiter.buckets);
        let _ = std::thread::spawn(move || {
            let _guard = buckets.lock().expect("lock buckets");
            panic!("intentional poison for test");
        })
        .join();
    }

    #[test]
    fn rate_limiter_fail_open_allows_when_lock_is_poisoned() {
        let limiter = InMemoryRateLimiter::new("test_fail_open", 10, Duration::from_secs(60), true);
        poison_limiter_lock(&limiter);

        let decision = limiter.check("k");
        assert!(decision.allowed);
        assert!(!decision.internal_error);
    }

    #[test]
    fn rate_limiter_fail_closed_blocks_when_lock_is_poisoned() {
        let limiter =
            InMemoryRateLimiter::new("test_fail_closed", 10, Duration::from_secs(60), false);
        poison_limiter_lock(&limiter);

        let decision = limiter.check("k");
        assert!(!decision.allowed);
        assert!(decision.internal_error);
        assert_eq!(decision.retry_after_secs, 1);
    }

    #[test]
    fn rate_limiter_failpoint_applies_to_selected_limiter() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvGuard::apply("true", Some("admin_endpoints"));

        assert!(should_simulate_rate_limiter_internal_error(
            "admin_endpoints"
        ));
        assert!(!should_simulate_rate_limiter_internal_error("events"));
    }

    #[test]
    fn rate_limiter_failpoint_fail_closed_returns_internal_error() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvGuard::apply("true", Some("admin_endpoints"));

        let limiter =
            InMemoryRateLimiter::new("admin_endpoints", 10, Duration::from_secs(60), false);
        let decision = limiter.check("k");
        assert!(!decision.allowed);
        assert!(decision.internal_error);
        assert_eq!(decision.retry_after_secs, 1);
    }

    #[test]
    fn distributed_denied_cache_returns_retry_window() {
        let cache = Mutex::new(HashMap::new());
        put_cached_denied_retry_secs(&cache, "k1", 3, 128);
        let retry_after = get_cached_denied_retry_secs(&cache, "k1").expect("expected cached deny");
        assert!((1..=3).contains(&retry_after));
    }

    #[test]
    fn distributed_denied_cache_evicted_when_expired() {
        let cache = Mutex::new(HashMap::new());
        {
            let mut guard = cache.lock().expect("cache lock");
            guard.insert("k2".to_string(), Instant::now() - Duration::from_secs(1));
        }
        assert!(get_cached_denied_retry_secs(&cache, "k2").is_none());
        let guard = cache.lock().expect("cache lock");
        assert!(!guard.contains_key("k2"));
    }

    #[test]
    fn rate_limit_key_prefers_authenticated_identity_scoped_by_org() {
        let mut req = Request::builder()
            .uri("/stats")
            .body(Body::empty())
            .expect("request");
        req.extensions_mut().insert(auth::AuthUser {
            client_id: "andres".to_string(),
            role: crate::models::UserRole::Admin,
            org_id: Some("org-123".to_string()),
        });

        let key = rate_limit_key_from_request(&req);
        assert_eq!(key, "org:org-123:user:andres");
    }

    #[test]
    fn rate_limit_key_uses_client_identity_when_org_missing() {
        let mut req = Request::builder()
            .uri("/stats")
            .body(Body::empty())
            .expect("request");
        req.extensions_mut().insert(auth::AuthUser {
            client_id: "andres".to_string(),
            role: crate::models::UserRole::Developer,
            org_id: None,
        });

        let key = rate_limit_key_from_request(&req);
        assert_eq!(key, "user:andres");
    }

    #[test]
    fn rate_limit_key_fallback_matches_ip_and_auth_fingerprint() {
        let req = Request::builder()
            .uri("/health")
            .header("x-real-ip", "10.20.30.40")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .expect("request");

        let digest = sha2::Sha256::digest("Bearer test-token".as_bytes());
        let expected_fingerprint = format!("{:x}", digest)[..12].to_string();
        let key = rate_limit_key_from_request(&req);
        assert_eq!(key, format!("10.20.30.40:{}", expected_fingerprint));
    }

    async fn inject_test_auth(mut req: Request<Body>, next: Next) -> Response {
        req.extensions_mut().insert(auth::AuthUser {
            client_id: "test-user".to_string(),
            role: crate::models::UserRole::Admin,
            org_id: Some("test-org".to_string()),
        });
        next.run(req).await
    }

    async fn attach_rate_limit_key_header(req: Request<Body>, next: Next) -> Response {
        let key = rate_limit_key_from_request(&req);
        let mut response = next.run(req).await;
        let value = HeaderValue::from_str(&key).expect("valid header value");
        response.headers_mut().insert("x-rate-limit-key", value);
        response
    }

    #[tokio::test]
    async fn auth_layer_populates_identity_before_route_level_rate_limit_key() {
        let app = Router::new()
            .route(
                "/probe",
                get(|| async { StatusCode::OK })
                    .layer(middleware::from_fn(attach_rate_limit_key_header)),
            )
            .layer(middleware::from_fn(inject_test_auth));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/probe")
                    .method("GET")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        let key = response
            .headers()
            .get("x-rate-limit-key")
            .and_then(|h| h.to_str().ok())
            .expect("x-rate-limit-key header");
        assert_eq!(key, "org:test-org:user:test-user");
    }
}
