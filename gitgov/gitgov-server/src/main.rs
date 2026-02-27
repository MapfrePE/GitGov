mod auth;
mod db;
mod handlers;
mod models;
mod notifications;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header::RETRY_AFTER, HeaderMap, HeaderValue, Request, StatusCode},
    middleware,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Router,
};
use clap::Parser;
use dotenvy::dotenv;
use sha2::Digest;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::handlers::AppState;
use crate::models::UserRole;

#[derive(Parser, Debug)]
#[command(name = "gitgov-server", about = "GitGov Control Plane")]
struct Args {
    #[arg(long, help = "Print bootstrap admin key to stdout (use for initial setup)")]
    print_bootstrap_key: bool,
}

const JOB_WORKER_TTL_SECS: u64 = 300;
const JOB_POLL_INTERVAL_SECS: u64 = 5;
const JOB_ERROR_BACKOFF_SECS: u64 = 10;

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
    buckets: Arc<Mutex<HashMap<String, RateBucket>>>,
}

#[derive(Debug)]
struct RateLimitDecision {
    allowed: bool,
    retry_after_secs: u64,
}

impl InMemoryRateLimiter {
    fn new(name: &'static str, limit: u32, window: Duration) -> Self {
        Self {
            name,
            limit,
            window,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn check(&self, key: &str) -> RateLimitDecision {
        if self.limit == 0 {
            return RateLimitDecision {
                allowed: true,
                retry_after_secs: 0,
            };
        }

        let now = Instant::now();
        let mut buckets = match self.buckets.lock() {
            Ok(guard) => guard,
            Err(_) => {
                // Fail-open to avoid breaking ingestion on poisoned lock.
                tracing::warn!(limiter = self.name, "Rate limiter lock poisoned; allowing request");
                return RateLimitDecision {
                    allowed: true,
                    retry_after_secs: 0,
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
            };
        }

        bucket.count += 1;
        RateLimitDecision {
            allowed: true,
            retry_after_secs: 0,
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

fn rate_limit_key_from_headers(headers: &HeaderMap) -> String {
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

async fn rate_limit_middleware(
    State(limiter): State<Arc<InMemoryRateLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let key = rate_limit_key_from_headers(req.headers());
    let decision = limiter.check(&key);

    if decision.allowed {
        return next.run(req).await;
    }

    tracing::warn!(
        limiter = limiter.name,
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

#[tokio::main]
async fn main() {
    dotenv().ok();
    
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gitgov_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let jwt_secret = std::env::var("GITGOV_JWT_SECRET")
        .unwrap_or_else(|_| "gitgov-secret-key-change-in-production".to_string());

    let github_webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET").ok();
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
    
    // Check if GITGOV_API_KEY is configured and insert it if not exists
    if let Ok(env_api_key) = std::env::var("GITGOV_API_KEY") {
        let key_hash = format!("{:x}", sha2::Sha256::digest(env_api_key.as_bytes()));
        
        // Check if this key already exists
        match db.validate_api_key(&key_hash).await {
            Ok(Some(_)) => {
                tracing::info!("GITGOV_API_KEY already exists in database");
            }
            Ok(None) => {
                // Key doesn't exist, insert it
                match db.create_api_key(&key_hash, "gitgov-desktop", None, &UserRole::Admin).await {
                    Ok(_) => {
                        tracing::info!("GITGOV_API_KEY inserted into database");
                        if should_print_key {
                            eprintln!();
                            eprintln!("╔════════════════════════════════════════════════════════════════╗");
                            eprintln!("║  GITGOV_API_KEY configured and ready                            ║");
                            eprintln!("╚════════════════════════════════════════════════════════════════╝");
                            eprintln!();
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to insert GITGOV_API_KEY: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to check GITGOV_API_KEY: {}", e);
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
    
    let worker_handle = tokio::spawn(async move {
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
                        "detect_signals" => {
                            db_for_worker.execute_detect_signals(&job.org_id).await
                                .map(|count| {
                                    tracing::info!(
                                        worker_id = %worker_id,
                                        job_id = %job.id,
                                        org_id = %job.org_id,
                                        signals_created = count,
                                        "Signal detection completed"
                                    );
                                })
                        }
                        job_type => {
                            tracing::warn!(
                                worker_id = %worker_id,
                                job_id = %job.id,
                                job_type = %job_type,
                                "Unknown job type"
                            );
                            Err(db::DbError::DatabaseError(format!("Unknown job type: {}", job_type)))
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
                            if let Err(err) = db_for_worker.fail_job(&job.id, &e.to_string()).await {
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
                    tokio::time::sleep(tokio::time::Duration::from_secs(JOB_POLL_INTERVAL_SECS)).await;
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
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client for notifications");

    let state = Arc::new(AppState {
        db: Arc::clone(&db),
        jwt_secret,
        github_webhook_secret,
        github_personal_access_token,
        jenkins_webhook_secret,
        jira_webhook_secret,
        start_time: Instant::now(),
        worker_id: worker_id_clone.clone(),
        http_client,
        alert_webhook_url,
    });
    
    let worker_id_for_log = worker_id_clone;

    let events_rate_limit = Arc::new(InMemoryRateLimiter::new(
        "events",
        parse_u32_env("GITGOV_RATE_LIMIT_EVENTS_PER_MIN", 240),
        Duration::from_secs(60),
    ));
    let audit_stream_rate_limit = Arc::new(InMemoryRateLimiter::new(
        "audit_stream",
        parse_u32_env("GITGOV_RATE_LIMIT_AUDIT_STREAM_PER_MIN", 60),
        Duration::from_secs(60),
    ));
    let jenkins_rate_limit = Arc::new(InMemoryRateLimiter::new(
        "jenkins_integrations",
        parse_u32_env("GITGOV_RATE_LIMIT_JENKINS_PER_MIN", 120),
        Duration::from_secs(60),
    ));
    let jira_rate_limit = Arc::new(InMemoryRateLimiter::new(
        "jira_integrations",
        parse_u32_env("GITGOV_RATE_LIMIT_JIRA_PER_MIN", 120),
        Duration::from_secs(60),
    ));
    let admin_rate_limit = Arc::new(InMemoryRateLimiter::new(
        "admin_endpoints",
        parse_u32_env("GITGOV_RATE_LIMIT_ADMIN_PER_MIN", 60),
        Duration::from_secs(60),
    ));
    let jenkins_body_limit_bytes = parse_usize_env(
        "GITGOV_JENKINS_MAX_BODY_BYTES",
        256 * 1024,
    );
    let jira_body_limit_bytes = parse_usize_env(
        "GITGOV_JIRA_MAX_BODY_BYTES",
        512 * 1024,
    );

    tracing::info!(
        events_per_min = events_rate_limit.limit,
        audit_stream_per_min = audit_stream_rate_limit.limit,
        jenkins_per_min = jenkins_rate_limit.limit,
        jira_per_min = jira_rate_limit.limit,
        jenkins_body_limit_bytes,
        jira_body_limit_bytes,
        admin_per_min = admin_rate_limit.limit,
        "Rate limiting enabled for ingestion and admin endpoints"
    );

    let auth_routes = Router::new()
        .route("/logs", get(handlers::get_logs).layer(middleware::from_fn_with_state(
            Arc::clone(&admin_rate_limit),
            rate_limit_middleware,
        )))
        .route("/stats", get(handlers::get_stats).layer(middleware::from_fn_with_state(
            Arc::clone(&admin_rate_limit),
            rate_limit_middleware,
        )))
        .route("/dashboard", get(handlers::get_dashboard).layer(middleware::from_fn_with_state(
            Arc::clone(&admin_rate_limit),
            rate_limit_middleware,
        )))
        .route(
            "/integrations/jenkins",
            post(handlers::ingest_jenkins_pipeline_event)
                .layer(DefaultBodyLimit::max(jenkins_body_limit_bytes))
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&jenkins_rate_limit),
                    rate_limit_middleware,
                )),
        )
        .route("/integrations/jenkins/status", get(handlers::get_jenkins_integration_status))
        .route("/integrations/jenkins/correlations", get(handlers::get_jenkins_commit_correlations))
        .route(
            "/integrations/jira",
            post(handlers::ingest_jira_webhook)
                .layer(DefaultBodyLimit::max(jira_body_limit_bytes))
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&jira_rate_limit),
                    rate_limit_middleware,
                )),
        )
        .route("/integrations/jira/status", get(handlers::get_jira_integration_status))
        .route("/integrations/jira/tickets/{ticket_id}", get(handlers::get_jira_ticket_detail))
        .route("/integrations/jira/correlate", post(handlers::correlate_jira_tickets))
        .route("/integrations/jira/ticket-coverage", get(handlers::get_jira_ticket_coverage))
        .route("/compliance/{org_name}", get(handlers::get_compliance_dashboard))
        .route("/signals", get(handlers::get_signals))
        .route("/signals/{signal_id}", post(handlers::update_signal))
        .route("/signals/{signal_id}/confirm", post(handlers::confirm_signal))
        .route("/signals/detect/{org_name}", post(handlers::trigger_detection))
        .route("/violations/{violation_id}/decisions", get(handlers::get_violation_decisions))
        .route("/violations/{violation_id}/decisions", post(handlers::add_violation_decision))
        .route("/policy/{repo_name}", get(handlers::get_policy))
        .route("/policy/check", post(handlers::policy_check))
        .route("/policy/{repo_name}/history", get(handlers::get_policy_history))
        .route("/policy/{repo_name}/override", put(handlers::override_policy))
        .route("/export", post(handlers::export_events))
        .route("/exports", get(handlers::list_exports))
        .route("/me", get(handlers::get_me))
        .route("/orgs", post(handlers::create_org))
        .route("/api-keys", get(handlers::list_api_keys).post(handlers::create_api_key))
        .route("/api-keys/{id}/revoke", post(handlers::revoke_api_key))
        .route(
            "/events",
            post(handlers::ingest_client_events).layer(middleware::from_fn_with_state(
                Arc::clone(&events_rate_limit),
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
        .route("/pr-merges", get(handlers::list_pr_merges).layer(middleware::from_fn_with_state(
            Arc::clone(&admin_rate_limit),
            rate_limit_middleware,
        )))
        .route("/admin-audit-log", get(handlers::list_admin_audit_log))
        .route("/jobs/metrics", get(handlers::get_job_metrics))
        .route("/jobs/dead", get(handlers::get_dead_jobs))
        .route("/jobs/{job_id}/retry", post(handlers::retry_dead_job))
        .layer(middleware::from_fn_with_state(Arc::clone(&db), auth::auth_middleware));

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/health/detailed", get(handlers::detailed_health))
        .route("/webhooks/github", post(handlers::handle_github_webhook))
        .merge(auth_routes)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = std::env::var("GITGOV_SERVER_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
        .parse()
        .expect("Invalid server address");

    tracing::info!("GitGov Control Plane starting on {}", addr);
    tracing::info!("Using Supabase PostgreSQL database");
    tracing::info!("");
    tracing::info!("Endpoints:");
    tracing::info!("  GET  /health                    - Health check (public)");
    tracing::info!("  GET  /health/detailed           - Detailed health (public)");
    tracing::info!("  POST /webhooks/github           - GitHub webhook (HMAC auth)");
    tracing::info!("  --- Authenticated endpoints ---");
    tracing::info!("  POST /events                    - Client events (auth)");
    tracing::info!("  GET  /logs                      - Query events (auth, dev: own only)");
    tracing::info!("  GET  /pr-merges                 - PR merge evidence (admin)");
    tracing::info!("  GET  /stats                     - Statistics (admin)");
    tracing::info!("  GET  /dashboard                 - Dashboard (admin)");
    tracing::info!("  POST /integrations/jenkins      - Jenkins pipeline ingest (admin)");
    tracing::info!("  GET  /integrations/jenkins/status - Jenkins integration health (admin)");
    tracing::info!("  GET  /integrations/jenkins/correlations - Commit->pipeline correlations (admin)");
    tracing::info!("  POST /integrations/jira         - Jira webhook ingest (admin)");
    tracing::info!("  GET  /integrations/jira/status  - Jira integration health (admin)");
    tracing::info!("  GET  /integrations/jira/tickets/:ticket_id - Jira ticket detail (admin)");
    tracing::info!("  POST /integrations/jira/correlate - Build commit<->ticket correlations (admin)");
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
    tracing::info!("  POST /policy/check              - Policy check (advisory, admin)");
    tracing::info!("  PUT  /policy/:repo/override     - Override policy (admin)");
    tracing::info!("  GET  /policy/:repo/history      - History (auth)");
    tracing::info!("  POST /export                    - Export (auth)");
    tracing::info!("  POST /orgs                      - Create/upsert org (admin)");
    tracing::info!("  POST /api-keys                  - Create API key (admin)");
    tracing::info!("  POST /audit-stream/github       - GitHub audit log stream (admin)");
    tracing::info!("  (opt) JENKINS_WEBHOOK_SECRET    - Extra shared secret header x-gitgov-jenkins-secret");
    tracing::info!("  (opt) JIRA_WEBHOOK_SECRET       - Extra shared secret header x-gitgov-jira-secret");
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

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        tracing::error!("Failed to bind to address {}: {}", addr, e);
        std::process::exit(1);
    });

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        tracing::error!("Server error: {}", e);
    });
}
