// Integration tests for GitGov Control Plane Server.
//
// These tests require a PostgreSQL database. Set TEST_DATABASE_URL to run them.
// The easiest way is to use docker-compose:
//
//   docker-compose up -d gitgov-db
//   TEST_DATABASE_URL=postgresql://gitgov:gitgov_dev_password@127.0.0.1:5433/gitgov cargo test integration
//
// Tests that cannot connect to the DB are skipped (not failed).

#[cfg(test)]
mod integration_tests {
    use crate::auth;
    use crate::db::Database;
    use crate::handlers::{self, AppState, ConversationalRuntime, PolicyCheckBlockingScope};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::{get, post},
        Router,
    };
    use sha2::Digest;
    use sqlx::PgPool;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicI64;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::sync::Semaphore;
    use tower::ServiceExt;

    /// Try to connect to the test database and set up an isolated schema.
    /// Returns None if TEST_DATABASE_URL is not set or connection fails (test will be skipped).
    /// Returns (pool_with_schema, schema_name, admin_pool_for_teardown).
    async fn try_setup() -> Option<(PgPool, String, PgPool)> {
        let url = std::env::var("TEST_DATABASE_URL").ok()?;

        // Admin pool: used to create/drop schema only.
        let admin_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&url)
            .await
            .ok()?;

        let schema = format!("test_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

        // Create schema using admin pool.
        sqlx::query(&format!("CREATE SCHEMA \"{}\"", schema))
            .execute(&admin_pool)
            .await
            .expect("create test schema");

        // Build a pool where EVERY connection sets search_path to the test schema.
        let schema_for_hook = schema.clone();
        let test_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(move |conn, _meta| {
                let s = schema_for_hook.clone();
                Box::pin(async move {
                    sqlx::query(&format!("SET search_path TO \"{}\"", s))
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .expect("connect test pool with schema");

        // Apply minimal DDL needed for the Golden Path tests.
        let ddl = r#"
            CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
            CREATE EXTENSION IF NOT EXISTS "pgcrypto";

            CREATE TABLE IF NOT EXISTS orgs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                github_id BIGINT UNIQUE,
                login TEXT UNIQUE NOT NULL,
                name TEXT,
                avatar_url TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS repos (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                github_id BIGINT UNIQUE,
                full_name TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                private BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS api_keys (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                key_hash TEXT UNIQUE NOT NULL,
                client_id TEXT NOT NULL,
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                role TEXT NOT NULL DEFAULT 'Developer',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                last_used TIMESTAMPTZ,
                is_active BOOLEAN DEFAULT TRUE
            );

            CREATE TABLE IF NOT EXISTS client_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
                event_uuid TEXT UNIQUE NOT NULL,
                event_type TEXT NOT NULL,
                user_login TEXT NOT NULL,
                user_name TEXT,
                branch TEXT,
                commit_sha TEXT,
                files JSONB DEFAULT '[]',
                status TEXT NOT NULL,
                reason TEXT,
                metadata JSONB DEFAULT '{}',
                client_version TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                synced_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS github_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
                delivery_id TEXT UNIQUE NOT NULL,
                event_type TEXT NOT NULL,
                actor_login TEXT,
                actor_id BIGINT,
                ref_name TEXT,
                ref_type TEXT,
                before_sha TEXT,
                after_sha TEXT,
                commit_shas JSONB DEFAULT '[]',
                commits_count INTEGER DEFAULT 0,
                payload JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                processed_at TIMESTAMPTZ
            );

            CREATE TABLE IF NOT EXISTS violations (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
                github_event_id UUID REFERENCES github_events(id),
                client_event_id UUID REFERENCES client_events(id),
                violation_type TEXT NOT NULL,
                severity TEXT DEFAULT 'warning',
                confidence_level TEXT DEFAULT 'pending',
                reason TEXT,
                user_login TEXT,
                branch TEXT,
                commit_sha TEXT,
                details JSONB DEFAULT '{}',
                correlated_github_event_id UUID REFERENCES github_events(id),
                correlated_client_event_id UUID REFERENCES client_events(id),
                resolved BOOLEAN DEFAULT FALSE,
                resolved_at TIMESTAMPTZ,
                resolved_by TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS policies (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                repo_id UUID REFERENCES repos(id) ON DELETE CASCADE UNIQUE,
                config JSONB NOT NULL,
                checksum TEXT NOT NULL,
                override_actor TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS webhook_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                delivery_id TEXT UNIQUE NOT NULL,
                event_type TEXT NOT NULL,
                payload JSONB NOT NULL,
                processed BOOLEAN DEFAULT FALSE,
                error TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS pipeline_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                pipeline_id TEXT NOT NULL,
                pipeline_name TEXT NOT NULL,
                status TEXT NOT NULL,
                branch TEXT,
                commit_sha TEXT,
                trigger_user TEXT,
                stages JSONB DEFAULT '[]',
                duration_ms BIGINT,
                url TEXT,
                metadata JSONB DEFAULT '{}',
                ingested_at TIMESTAMPTZ DEFAULT NOW(),
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS project_tickets (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                ticket_id TEXT UNIQUE NOT NULL,
                project_key TEXT NOT NULL,
                summary TEXT,
                status TEXT,
                assignee TEXT,
                reporter TEXT,
                ticket_type TEXT,
                priority TEXT,
                labels JSONB DEFAULT '[]',
                related_commits JSONB DEFAULT '[]',
                related_prs JSONB DEFAULT '[]',
                url TEXT,
                raw_payload JSONB DEFAULT '{}',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS commit_ticket_correlations (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                commit_sha TEXT NOT NULL,
                ticket_id TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(commit_sha, ticket_id)
            );

            CREATE TABLE IF NOT EXISTS export_logs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                requested_by TEXT NOT NULL,
                format TEXT NOT NULL,
                filters JSONB DEFAULT '{}',
                event_count INTEGER DEFAULT 0,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS governance_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                event_type TEXT NOT NULL,
                actor TEXT,
                repo TEXT,
                branch TEXT,
                details JSONB DEFAULT '{}',
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS pr_merges (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                repo TEXT NOT NULL,
                pr_number INTEGER NOT NULL,
                title TEXT,
                author TEXT,
                merged_by TEXT,
                base_branch TEXT,
                head_branch TEXT,
                commit_sha TEXT,
                reviewers JSONB DEFAULT '[]',
                approved_by JSONB DEFAULT '[]',
                review_count INTEGER DEFAULT 0,
                additions INTEGER DEFAULT 0,
                deletions INTEGER DEFAULT 0,
                changed_files INTEGER DEFAULT 0,
                url TEXT,
                merged_at TIMESTAMPTZ DEFAULT NOW(),
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS admin_audit_log (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT,
                details JSONB DEFAULT '{}',
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS client_sessions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                client_id TEXT NOT NULL,
                org_id UUID,
                app_version TEXT,
                os TEXT,
                hostname TEXT,
                last_seen TIMESTAMPTZ DEFAULT NOW(),
                created_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(client_id)
            );

            CREATE TABLE IF NOT EXISTS identity_aliases (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                primary_login TEXT NOT NULL,
                alias_login TEXT UNIQUE NOT NULL,
                created_by TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS noncompliance_signals (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID,
                signal_type TEXT NOT NULL,
                severity TEXT DEFAULT 'medium',
                status TEXT DEFAULT 'open',
                description TEXT,
                evidence JSONB DEFAULT '{}',
                user_login TEXT,
                repo TEXT,
                branch TEXT,
                commit_sha TEXT,
                detected_at TIMESTAMPTZ DEFAULT NOW(),
                reviewed_at TIMESTAMPTZ,
                reviewed_by TEXT,
                resolution TEXT,
                violation_id UUID,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS violation_decisions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                violation_id UUID NOT NULL,
                actor TEXT NOT NULL,
                decision TEXT NOT NULL,
                reason TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS policy_history (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                repo_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                config JSONB,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS policy_drift_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                user_login TEXT NOT NULL,
                action TEXT NOT NULL,
                repo_name TEXT NOT NULL,
                result TEXT NOT NULL,
                before_checksum TEXT,
                after_checksum TEXT,
                duration_ms BIGINT,
                metadata JSONB DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS policy_change_requests (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
                repo_name TEXT NOT NULL,
                requested_by TEXT NOT NULL,
                requested_config JSONB NOT NULL,
                requested_checksum TEXT NOT NULL,
                reason TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS policy_change_request_decisions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                request_id UUID UNIQUE REFERENCES policy_change_requests(id) ON DELETE CASCADE,
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                decision TEXT NOT NULL,
                decided_by TEXT NOT NULL,
                note TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                job_type TEXT NOT NULL,
                payload JSONB DEFAULT '{}',
                status TEXT NOT NULL DEFAULT 'pending',
                attempts INTEGER DEFAULT 0,
                max_attempts INTEGER DEFAULT 3,
                error TEXT,
                worker_id TEXT,
                locked_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS org_users (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID NOT NULL,
                login TEXT NOT NULL,
                display_name TEXT,
                email TEXT,
                role TEXT NOT NULL DEFAULT 'Developer',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(org_id, login)
            );

            CREATE TABLE IF NOT EXISTS org_invitations (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                org_id UUID NOT NULL,
                email TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'Developer',
                token_hash TEXT UNIQUE NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                invited_by TEXT NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL,
                accepted_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS feature_requests (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                user_login TEXT NOT NULL,
                org_id TEXT,
                title TEXT NOT NULL,
                description TEXT,
                category TEXT DEFAULT 'general',
                priority TEXT DEFAULT 'normal',
                status TEXT DEFAULT 'open',
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            -- Indexes for performance
            CREATE INDEX IF NOT EXISTS idx_client_events_uuid ON client_events(event_uuid);
            CREATE INDEX IF NOT EXISTS idx_client_events_created ON client_events(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_client_events_type ON client_events(event_type);
            CREATE INDEX IF NOT EXISTS idx_client_events_user ON client_events(user_login);
            CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);
        "#;

        sqlx::raw_sql(ddl)
            .execute(&test_pool)
            .await
            .expect("apply test DDL");

        Some((test_pool, schema, admin_pool))
    }

    /// Drop the test schema after the test. Uses admin_pool (no search_path override).
    async fn teardown(admin_pool: &PgPool, schema: &str) {
        let _ = sqlx::query(&format!("DROP SCHEMA \"{}\" CASCADE", schema))
            .execute(admin_pool)
            .await;
    }

    /// Insert a test API key into the database. Returns the raw key.
    async fn insert_test_api_key(pool: &PgPool, client_id: &str, role: &str) -> String {
        let raw_key = format!("test-key-{}", uuid::Uuid::new_v4());
        let hash = format!("{:x}", sha2::Sha256::digest(raw_key.as_bytes()));
        sqlx::query(
            "INSERT INTO api_keys (key_hash, client_id, role, is_active) VALUES ($1, $2, $3, true)",
        )
        .bind(&hash)
        .bind(client_id)
        .bind(role)
        .execute(pool)
        .await
        .expect("insert test API key");
        raw_key
    }

    /// Insert a minimal org + repo for policy endpoints.
    async fn insert_test_repo(pool: &PgPool, full_name: &str) -> (String, String) {
        let org_id = uuid::Uuid::new_v4().to_string();
        let repo_id = uuid::Uuid::new_v4().to_string();
        let org_login = format!("org-{}", uuid::Uuid::new_v4().simple());
        let repo_name = full_name.split('/').nth(1).unwrap_or("repo").to_string();

        sqlx::query("INSERT INTO orgs (id, login, name) VALUES ($1::uuid, $2, $3)")
            .bind(&org_id)
            .bind(&org_login)
            .bind("Test Org")
            .execute(pool)
            .await
            .expect("insert test org");

        sqlx::query(
            "INSERT INTO repos (id, org_id, full_name, name, private) VALUES ($1::uuid, $2::uuid, $3, $4, false)",
        )
        .bind(&repo_id)
        .bind(&org_id)
        .bind(full_name)
        .bind(&repo_name)
        .execute(pool)
        .await
        .expect("insert test repo");

        (org_id, repo_id)
    }

    async fn insert_test_policy(pool: &PgPool, repo_id: &str, config: serde_json::Value) {
        sqlx::query(
            r#"
            INSERT INTO policies (id, org_id, repo_id, config, checksum, override_actor)
            SELECT
                gen_random_uuid(),
                r.org_id,
                r.id,
                $2::jsonb,
                $3,
                'integration-test'
            FROM repos r
            WHERE r.id = $1::uuid
            ON CONFLICT (repo_id) DO UPDATE
            SET config = EXCLUDED.config,
                checksum = EXCLUDED.checksum,
                updated_at = NOW()
            "#,
        )
        .bind(repo_id)
        .bind(config)
        .bind(format!("checksum-{}", uuid::Uuid::new_v4()))
        .execute(pool)
        .await
        .expect("insert test policy");
    }

    /// Build a minimal Router with auth middleware for integration testing.
    fn build_test_app_with_options(
        db: Arc<Database>,
        alert_webhook_url: Option<String>,
        drift_alert_webhook_urls: Vec<String>,
        policy_check_block_scopes: Vec<PolicyCheckBlockingScope>,
    ) -> Router {
        let state = AppState {
            db: Arc::clone(&db),
            github_webhook_secret: None,
            github_personal_access_token: None,
            jenkins_webhook_secret: None,
            jira_webhook_secret: None,
            start_time: Instant::now(),
            worker_id: "test-worker".to_string(),
            http_client: reqwest::Client::new(),
            alert_webhook_url,
            drift_alert_webhook_urls,
            strict_actor_match: false,
            reject_synthetic_logins: false,
            events_max_batch: 1000,
            llm_api_key: None,
            llm_model: "test".to_string(),
            feature_request_webhook_url: None,
            conversational_runtime: Arc::new(Mutex::new(ConversationalRuntime::default())),
            chat_llm_semaphore: Arc::new(Semaphore::new(1)),
            chat_llm_queue_timeout_ms: 500,
            chat_llm_timeout_ms: 9000,
            stats_cache_ttl: Duration::from_millis(100),
            stats_cache: Arc::new(Mutex::new(HashMap::new())),
            org_lookup_cache_ttl: Duration::from_millis(0),
            org_lookup_cache: Arc::new(Mutex::new(HashMap::new())),
            repo_lookup_cache_ttl: Duration::from_millis(0),
            repo_lookup_cache: Arc::new(Mutex::new(HashMap::new())),
            repo_upsert_min_interval: Duration::from_millis(0),
            repo_upsert_last_attempt: Arc::new(Mutex::new(HashMap::new())),
            cache_invalidation_min_interval: Duration::from_millis(0),
            stats_cache_invalidation_min_interval: Duration::from_millis(0),
            logs_cache_invalidation_min_interval: Duration::from_millis(0),
            stats_cache_last_invalidation_ms: Arc::new(AtomicI64::new(0)),
            stats_cache_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
            logs_cache_ttl: Duration::from_millis(100),
            logs_cache_stale_on_error: Duration::from_millis(1000),
            logs_reject_offset_pagination: false,
            outbox_server_lease_enabled: false,
            outbox_server_lease_ttl_ms: 2000,
            outbox_lease_telemetry: Arc::new(Mutex::new(handlers::OutboxLeaseTelemetry::default())),
            logs_cache: Arc::new(Mutex::new(HashMap::new())),
            logs_cache_last_invalidation_ms: Arc::new(AtomicI64::new(0)),
            client_session_upsert_min_interval: Duration::from_millis(0),
            client_session_last_upsert: Arc::new(Mutex::new(HashMap::new())),
            sse_tx: tokio::sync::broadcast::channel::<handlers::SseNotification>(64).0,
            sse_max_connections: Arc::new(Semaphore::new(50)),
            sse_distributed_enabled: false,
            sse_distributed_channel: "test_sse".to_string(),
            policy_check_block_scopes,
        };

        let auth_routes = Router::new()
            .route("/events", post(handlers::ingest_client_events))
            .route("/logs", get(handlers::get_logs))
            .route("/stats", get(handlers::get_stats))
            .route("/stats/daily", get(handlers::get_daily_activity))
            .route("/dashboard", get(handlers::get_dashboard))
            .route(
                "/compliance/{org_name}",
                get(handlers::get_compliance_dashboard),
            )
            .route(
                "/signals/detect/{org_name}",
                post(handlers::trigger_detection),
            )
            .route("/me", get(handlers::get_me))
            .route("/orgs", post(handlers::create_org))
            .route("/export", post(handlers::export_events))
            .route("/policy/{repo_name}", get(handlers::get_policy))
            .route("/policy/check", post(handlers::policy_check))
            .route(
                "/policy/{repo_name}/requests",
                post(handlers::create_policy_change_request)
                    .get(handlers::list_policy_change_requests),
            )
            .route(
                "/policy/requests/{request_id}/approve",
                post(handlers::approve_policy_change_request),
            )
            .route(
                "/policy/requests/{request_id}/reject",
                post(handlers::reject_policy_change_request),
            )
            .route(
                "/policy/drift-events",
                post(handlers::ingest_policy_drift_event).get(handlers::list_policy_drift_events),
            )
            .layer(middleware::from_fn_with_state(
                Arc::clone(&db),
                auth::auth_middleware,
            ));

        Router::new()
            .route("/health", get(handlers::health))
            .route("/health/detailed", get(handlers::detailed_health))
            .merge(auth_routes)
            .with_state(Arc::new(state))
    }

    fn build_test_app_with_alerts(
        db: Arc<Database>,
        alert_webhook_url: Option<String>,
        drift_alert_webhook_urls: Vec<String>,
    ) -> Router {
        build_test_app_with_options(db, alert_webhook_url, drift_alert_webhook_urls, vec![])
    }

    fn build_test_app_with_policy_check_scopes(
        db: Arc<Database>,
        policy_check_block_scopes: Vec<PolicyCheckBlockingScope>,
    ) -> Router {
        build_test_app_with_options(db, None, vec![], policy_check_block_scopes)
    }

    fn build_test_app(db: Arc<Database>) -> Router {
        build_test_app_with_options(db, None, vec![], vec![])
    }

    /// Helper: make a JSON request to the test app.
    async fn json_request(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<&str>,
        api_key: Option<&str>,
    ) -> (StatusCode, String) {
        let mut builder = Request::builder().uri(uri);
        builder = match method {
            "GET" => builder.method("GET"),
            "POST" => builder.method("POST"),
            _ => builder.method(method),
        };
        if let Some(key) = api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }
        if body.is_some() {
            builder = builder.header("Content-Type", "application/json");
        }
        let req_body = body
            .map(|b| Body::from(b.to_string()))
            .unwrap_or(Body::empty());
        let request = builder.body(req_body).unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes).to_string();
        (status, body_str)
    }

    async fn spawn_webhook_probe() -> (
        String,
        tokio::sync::oneshot::Receiver<String>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind webhook probe listener");
        let addr = listener.local_addr().expect("listener local addr");
        let (body_tx, body_rx) = tokio::sync::oneshot::channel::<String>();
        let task = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let (mut socket, _) = listener.accept().await.expect("accept webhook connection");
            let mut buf = vec![0u8; 16 * 1024];
            let read = socket.read(&mut buf).await.expect("read webhook request");
            let req = String::from_utf8_lossy(&buf[..read]).to_string();
            let body = req.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
            let _ = body_tx.send(body);
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await;
        });

        (format!("http://{}", addr), body_rx, task)
    }

    /// Macro to reduce boilerplate: skip test if DB unavailable.
    macro_rules! setup_or_skip {
        () => {
            match try_setup().await {
                Some(result) => result,
                None => {
                    eprintln!("SKIPPED: TEST_DATABASE_URL not set or unreachable");
                    return;
                }
            }
        };
    }

    // ========================================================================
    // TESTS
    // ========================================================================

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (status, body) = json_request(&app, "GET", "/health", None, None).await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["status"], "ok");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn health_detailed_returns_database_info() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (status, body) = json_request(&app, "GET", "/health/detailed", None, None).await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert!(parsed["database"].is_object());

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn unauthenticated_request_returns_401() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (status, _) = json_request(&app, "GET", "/stats", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = json_request(&app, "GET", "/logs", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = json_request(&app, "GET", "/me", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn invalid_api_key_returns_401() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (status, _) =
            json_request(&app, "GET", "/stats", None, Some("invalid-key-12345")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn authenticated_me_returns_user_info() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (status, body) = json_request(&app, "GET", "/me", None, Some(&api_key)).await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["client_id"], "test-admin");
        assert_eq!(parsed["role"], "Admin");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn trigger_detection_falls_back_when_legacy_sql_detector_errors() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let org_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO orgs (id, login, name) VALUES ($1::uuid, $2, $3)")
            .bind(&org_id)
            .bind("acme")
            .bind("Acme Inc")
            .execute(&pool)
            .await
            .expect("insert org for detect test");

        // Force legacy detector to fail and ensure HTTP path remains resilient.
        sqlx::query(
            r#"
            CREATE OR REPLACE FUNCTION detect_noncompliance_signals(
                p_org_id UUID,
                p_window_minutes INTEGER DEFAULT 15,
                p_tolerance_minutes INTEGER DEFAULT 30
            ) RETURNS INTEGER AS $$
            BEGIN
                RAISE EXCEPTION 'forced legacy detector failure';
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .execute(&pool)
        .await
        .expect("override legacy detector function");

        let (status, body) =
            json_request(&app, "POST", "/signals/detect/acme", None, Some(&api_key)).await;

        assert_eq!(status, StatusCode::OK, "detect endpoint failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed["signals_created"].is_number());

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn compliance_dashboard_includes_monthly_timeline_points() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let org_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO orgs (id, login, name) VALUES ($1::uuid, $2, $3)")
            .bind(&org_id)
            .bind("acme")
            .bind("Acme Inc")
            .execute(&pool)
            .await
            .expect("insert org");

        sqlx::query(
            r#"
            CREATE OR REPLACE FUNCTION get_compliance_dashboard(p_org_id UUID)
            RETURNS JSON AS $$
            BEGIN
                RETURN json_build_object(
                    'signals', json_build_object(
                        'total', 0,
                        'pending', 0,
                        'high_confidence', 0,
                        'by_type', '{}'::json
                    ),
                    'correlation', json_build_object(
                        'github_pushes_24h', 0,
                        'client_pushes_24h', 0,
                        'correlation_rate', 1.0
                    ),
                    'policy', json_build_object(
                        'repos_with_policy', 0,
                        'total_repos', 0,
                        'recent_changes', 0
                    ),
                    'exports', json_build_object(
                        'total', 0,
                        'last_7_days', 0
                    )
                );
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .execute(&pool)
        .await
        .expect("create test compliance dashboard function");

        let repo_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO repos (id, org_id, full_name, name) VALUES ($1::uuid, $2::uuid, $3, $4)",
        )
        .bind(&repo_id)
        .bind(&org_id)
        .bind("acme/repo")
        .bind("repo")
        .execute(&pool)
        .await
        .expect("insert repo");

        sqlx::query(
            r#"
            INSERT INTO client_events (
                id, org_id, repo_id, event_uuid, event_type, user_login, commit_sha, status, created_at
            ) VALUES (
                gen_random_uuid(), $1::uuid, $2::uuid, $3, 'commit', 'test-admin', 'deadbeef', 'success',
                NOW() - INTERVAL '5 days'
            )
            "#,
        )
        .bind(&org_id)
        .bind(&repo_id)
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .expect("insert commit event");

        sqlx::query(
            r#"
            INSERT INTO commit_ticket_correlations (
                id, org_id, commit_sha, ticket_id, source, created_at
            ) VALUES (
                gen_random_uuid(), $1::uuid, 'deadbeef', 'ACME-1', 'test', NOW() - INTERVAL '5 days'
            )
            "#,
        )
        .bind(&org_id)
        .execute(&pool)
        .await
        .expect("insert commit-ticket correlation");

        sqlx::query(
            r#"
            INSERT INTO pipeline_events (
                id, org_id, pipeline_id, pipeline_name, status, commit_sha, branch, ingested_at, created_at
            ) VALUES (
                gen_random_uuid(), $1::uuid, 'p-1', 'job/main', 'success', 'deadbeef', 'main',
                NOW() - INTERVAL '5 days', NOW() - INTERVAL '5 days'
            )
            "#,
        )
        .bind(&org_id)
        .execute(&pool)
        .await
        .expect("insert pipeline event");

        sqlx::query(
            r#"
            INSERT INTO noncompliance_signals (
                id, org_id, signal_type, status, created_at
            ) VALUES (
                gen_random_uuid(), $1::uuid, 'commit_no_ticket', 'pending',
                NOW() - INTERVAL '5 days'
            )
            "#,
        )
        .bind(&org_id)
        .execute(&pool)
        .await
        .expect("insert signal");

        sqlx::query(
            r#"
            INSERT INTO violations (
                id, org_id, repo_id, violation_type, created_at
            ) VALUES (
                gen_random_uuid(), $1::uuid, $2::uuid, 'policy_violation',
                NOW() - INTERVAL '5 days'
            )
            "#,
        )
        .bind(&org_id)
        .bind(&repo_id)
        .execute(&pool)
        .await
        .expect("insert violation");

        let (status, body) =
            json_request(&app, "GET", "/compliance/acme", None, Some(&api_key)).await;

        assert_eq!(status, StatusCode::OK, "compliance failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let timeline = parsed["timeline"].as_array().expect("timeline array");
        assert_eq!(timeline.len(), 6, "expected 6 monthly points");
        assert!(timeline
            .iter()
            .any(|item| item["commits_total"].as_i64().unwrap_or(0) >= 1));
        assert!(timeline
            .iter()
            .any(|item| item["pipeline_runs_total"].as_i64().unwrap_or(0) >= 1));

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn golden_path_ingest_events_and_query() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        // Step 1: Ingest events (Golden Path: stage → commit → push)
        let events_payload = serde_json::json!({
            "events": [
                {
                    "event_uuid": "aaaaaaaa-0000-0000-0000-000000000001",
                    "event_type": "stage_files",
                    "user_login": "test-admin",
                    "files": [{"path": "src/main.rs", "status": "modified"}],
                    "status": "success",
                    "timestamp": 1700000000
                },
                {
                    "event_uuid": "aaaaaaaa-0000-0000-0000-000000000002",
                    "event_type": "commit",
                    "user_login": "test-admin",
                    "files": [{"path": "src/main.rs", "status": "modified"}],
                    "status": "success",
                    "branch": "main",
                    "commit_sha": "abc123def456",
                    "timestamp": 1700000001
                },
                {
                    "event_uuid": "aaaaaaaa-0000-0000-0000-000000000003",
                    "event_type": "successful_push",
                    "user_login": "test-admin",
                    "files": [],
                    "status": "success",
                    "branch": "main",
                    "timestamp": 1700000002
                }
            ],
            "client_version": "integration-test"
        });

        let (status, body) = json_request(
            &app,
            "POST",
            "/events",
            Some(&events_payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "ingest failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["accepted"].as_array().unwrap().len(), 3);
        assert_eq!(parsed["duplicates"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["errors"].as_array().unwrap().len(), 0);

        // Step 2: Query logs — should see the 3 events
        let (status, body) =
            json_request(&app, "GET", "/logs?limit=10&offset=0", None, Some(&api_key)).await;
        assert_eq!(status, StatusCode::OK, "logs failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let events = parsed["events"].as_array().unwrap();
        assert!(
            events.len() >= 3,
            "expected ≥3 events, got {}",
            events.len()
        );

        // Step 3: Query stats — should reflect the ingested data
        let (status, body) = json_request(&app, "GET", "/stats", None, Some(&api_key)).await;
        assert_eq!(status, StatusCode::OK, "stats failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(
            parsed["total_events"].as_i64().unwrap() >= 3,
            "expected total_events ≥ 3"
        );

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn event_deduplication_works() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let event = serde_json::json!({
            "events": [{
                "event_uuid": "dedup-test-uuid-001",
                "event_type": "commit",
                "user_login": "test-admin",
                "files": [],
                "status": "success",
                "timestamp": 1700000000
            }],
            "client_version": "integration-test"
        });

        // First ingestion — accepted
        let (status, body) = json_request(
            &app,
            "POST",
            "/events",
            Some(&event.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["accepted"].as_array().unwrap().len(), 1);

        // Second ingestion — deduplicated
        let (status, body) = json_request(
            &app,
            "POST",
            "/events",
            Some(&event.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["accepted"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["duplicates"].as_array().unwrap().len(), 1);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn developer_role_cannot_access_admin_endpoints() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let dev_key = insert_test_api_key(&pool, "test-dev", "Developer").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        // Developer can access /me
        let (status, body) = json_request(&app, "GET", "/me", None, Some(&dev_key)).await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["role"], "Developer");

        // Developer cannot access /stats (admin-only)
        let (status, _) = json_request(&app, "GET", "/stats", None, Some(&dev_key)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Developer cannot access /dashboard (admin-only)
        let (status, _) = json_request(&app, "GET", "/dashboard", None, Some(&dev_key)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn create_org_requires_founder_global_admin_key() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let founder_key = insert_test_api_key(&pool, "bootstrap-admin", "Admin").await;
        let non_founder_admin_key = insert_test_api_key(&pool, "admin-user", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let payload = serde_json::json!({
            "login": "scope-test-org",
            "name": "Scope Test Org"
        });

        let (status, body) = json_request(
            &app,
            "POST",
            "/orgs",
            Some(&payload.to_string()),
            Some(&non_founder_admin_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "non-founder admin should be blocked: {}",
            body
        );

        let (status, body) = json_request(
            &app,
            "POST",
            "/orgs",
            Some(&payload.to_string()),
            Some(&founder_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "founder should be allowed: {}",
            body
        );
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["login"], "scope-test-org");
        assert_eq!(parsed["created"], true);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn create_api_key_rejects_invalid_role_instead_of_silent_fallback() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "bootstrap-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let invalid_payload = serde_json::json!({
            "client_id": "role-case-test",
            "role": "admin"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/api-keys",
            Some(&invalid_payload.to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected body: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            parsed["error"],
            "role must be one of: Admin, Architect, Developer, PM"
        );

        let valid_payload = serde_json::json!({
            "client_id": "role-case-test-valid",
            "role": "Admin"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/api-keys",
            Some(&valid_payload.to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "unexpected body: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(
            parsed["api_key"].as_str().is_some(),
            "api_key should be present for valid role"
        );
        assert_eq!(parsed["client_id"], "role-case-test-valid");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn developer_only_sees_own_logs() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "admin-user", "Admin").await;
        let dev_key = insert_test_api_key(&pool, "dev-user", "Developer").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        // Admin ingests events for two different users
        let events = serde_json::json!({
            "events": [
                {
                    "event_uuid": "scope-test-001",
                    "event_type": "commit",
                    "user_login": "admin-user",
                    "files": [],
                    "status": "success",
                    "timestamp": 1700000000
                },
                {
                    "event_uuid": "scope-test-002",
                    "event_type": "commit",
                    "user_login": "dev-user",
                    "files": [],
                    "status": "success",
                    "timestamp": 1700000001
                },
                {
                    "event_uuid": "scope-test-003",
                    "event_type": "commit",
                    "user_login": "other-user",
                    "files": [],
                    "status": "success",
                    "timestamp": 1700000002
                }
            ],
            "client_version": "integration-test"
        });

        let (status, _) = json_request(
            &app,
            "POST",
            "/events",
            Some(&events.to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Developer queries logs — should only see their own events
        let (status, body) =
            json_request(&app, "GET", "/logs?limit=50&offset=0", None, Some(&dev_key)).await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let events = parsed["events"].as_array().unwrap();
        for event in events {
            let source = &event["source"];
            if source.is_string() && source.as_str().unwrap() == "client" {
                if let Some(login) = event["user_login"].as_str() {
                    assert_eq!(login, "dev-user", "Developer saw another user's event");
                }
            }
        }

        // Admin queries logs — should see all events
        let (status, body) = json_request(
            &app,
            "GET",
            "/logs?limit=50&offset=0",
            None,
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let events = parsed["events"].as_array().unwrap();
        assert!(events.len() >= 3, "admin should see all 3 events");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn events_endpoint_validates_payload() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        // Empty events array
        let empty = serde_json::json!({ "events": [], "client_version": "test" });
        let (status, _) = json_request(
            &app,
            "POST",
            "/events",
            Some(&empty.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Malformed JSON
        let (status, _) = json_request(
            &app,
            "POST",
            "/events",
            Some("not json at all"),
            Some(&api_key),
        )
        .await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
            "expected 400 or 422 for malformed JSON, got {}",
            status
        );

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn daily_activity_endpoint_returns_data() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "test-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (status, body) = json_request(&app, "GET", "/stats/daily", None, Some(&api_key)).await;
        assert_eq!(status, StatusCode::OK, "daily activity failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed.is_array(), "expected array response");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_check_is_advisory_by_default_even_when_not_allowed() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let (_, repo_id) = insert_test_repo(&pool, "acme/repo").await;
        let mut config = crate::models::GitGovConfig::default();
        config.branches.patterns = vec!["feature/*".to_string()];
        config.enforcement.branches = crate::models::EnforcementLevel::Block;
        let config_json = serde_json::to_value(config).expect("serialize policy config");
        insert_test_policy(&pool, &repo_id, config_json).await;

        let payload = serde_json::json!({
            "repo": "acme/repo",
            "branch": "main",
            "user_login": "policy-admin"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/check",
            Some(&payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "expected advisory 200: {}", body);
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("parse policy check body");
        assert_eq!(parsed["allowed"], false);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_check_returns_conflict_when_block_scope_matches_org_and_branch() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app_with_policy_check_scopes(
            db,
            vec![PolicyCheckBlockingScope::new(
                "acme".to_string(),
                "main".to_string(),
            )],
        );

        let (_, repo_id) = insert_test_repo(&pool, "acme/repo").await;
        let mut config = crate::models::GitGovConfig::default();
        config.branches.patterns = vec!["feature/*".to_string()];
        config.enforcement.branches = crate::models::EnforcementLevel::Block;
        let config_json = serde_json::to_value(config).expect("serialize policy config");
        insert_test_policy(&pool, &repo_id, config_json).await;

        let payload = serde_json::json!({
            "repo": "acme/repo",
            "branch": "main",
            "user_login": "policy-admin"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/check",
            Some(&payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "expected blocking 409: {}",
            body
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("parse policy check body");
        assert_eq!(parsed["allowed"], false);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_drift_events_require_auth() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);
        let payload = serde_json::json!({
            "action": "sync_local",
            "repo_name": "acme-repo",
            "result": "success"
        });

        let (status, _) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&payload.to_string()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = json_request(&app, "GET", "/policy/drift-events", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_drift_ingest_and_list_for_admin() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);
        let payload = serde_json::json!({
            "action": "sync_local",
            "repo_name": "acme-repo",
            "result": "success",
            "before_checksum": "abc",
            "after_checksum": "def",
            "duration_ms": 42,
            "metadata": { "source": "integration-test" }
        });

        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "ingest failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["accepted"], true);
        assert!(parsed["id"].is_string());

        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/drift-events?limit=10&offset=0",
            None,
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "list failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let events = parsed["events"].as_array().unwrap();
        assert_eq!(parsed["total"].as_i64().unwrap(), 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["user_login"], "policy-admin");
        assert_eq!(events[0]["action"], "sync_local");
        assert_eq!(events[0]["repo_name"], "acme-repo");
        assert_eq!(events[0]["result"], "success");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_drift_rejects_invalid_payload() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let invalid_action = serde_json::json!({
            "action": "invalid",
            "repo_name": "acme-repo",
            "result": "success"
        });
        let (status, _) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&invalid_action.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let empty_repo = serde_json::json!({
            "action": "sync_local",
            "repo_name": "",
            "result": "success"
        });
        let (status, _) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&empty_repo.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_drift_scope_is_enforced_for_developer() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "admin-user", "Admin").await;
        let dev_key = insert_test_api_key(&pool, "dev-user", "Developer").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let admin_event = serde_json::json!({
            "action": "push_local",
            "repo_name": "repo-admin",
            "result": "success"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&admin_event.to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "admin ingest failed: {}", body);

        let dev_event = serde_json::json!({
            "action": "sync_local",
            "repo_name": "repo-dev",
            "result": "failed"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&dev_event.to_string()),
            Some(&dev_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "dev ingest failed: {}", body);

        // Developer cannot expand scope through query params.
        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/drift-events?limit=50&offset=0&user_login=admin-user",
            None,
            Some(&dev_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "dev list failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let events = parsed["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["user_login"], "dev-user");
        assert_eq!(events[0]["repo_name"], "repo-dev");

        // Admin can filter explicitly.
        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/drift-events?limit=50&offset=0&user_login=admin-user",
            None,
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "admin list failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let events = parsed["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["user_login"], "admin-user");
        assert_eq!(events[0]["repo_name"], "repo-admin");

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn critical_drift_alert_is_dispatched_to_dedicated_webhook() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "drift-alert-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));

        let (probe_url, body_rx, probe_task) = spawn_webhook_probe().await;
        let app = build_test_app_with_alerts(db, None, vec![probe_url]);

        let repo_name = format!("org/repo-{}", uuid::Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "action": "drift_snapshot",
            "repo_name": repo_name,
            "result": "observed",
            "metadata": {
                "drift_count": 4,
                "critical_count": 2
            }
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "drift ingest failed: {}", body);

        let delivered_body = tokio::time::timeout(Duration::from_secs(2), body_rx)
            .await
            .expect("webhook delivery timeout")
            .expect("webhook body channel");
        assert!(
            delivered_body.contains("\"text\""),
            "expected slack-compatible text payload"
        );
        assert!(
            delivered_body.contains("Policy Drift"),
            "expected drift alert text in webhook payload"
        );
        assert!(
            delivered_body.contains("drift-alert-admin"),
            "expected actor login in webhook payload"
        );

        probe_task.await.expect("webhook probe task");
        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn critical_drift_alert_falls_back_to_generic_webhook() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "drift-fallback-admin", "Admin").await;
        let db = Arc::new(Database::from_pool(pool.clone()));

        let (probe_url, body_rx, probe_task) = spawn_webhook_probe().await;
        let app = build_test_app_with_alerts(db, Some(probe_url), vec![]);

        let repo_name = format!("org/repo-{}", uuid::Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "action": "drift_snapshot",
            "repo_name": repo_name,
            "result": "observed",
            "metadata": {
                "drift_count": 3,
                "critical_count": 1
            }
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "drift ingest failed: {}", body);

        let delivered_body = tokio::time::timeout(Duration::from_secs(2), body_rx)
            .await
            .expect("fallback webhook delivery timeout")
            .expect("fallback webhook body channel");
        assert!(
            delivered_body.contains("Policy Drift"),
            "expected drift alert text in fallback webhook payload"
        );
        assert!(
            delivered_body.contains("drift-fallback-admin"),
            "expected actor login in fallback webhook payload"
        );

        probe_task.await.expect("fallback webhook probe task");
        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn export_includes_policy_drift_and_policy_requests_in_json_and_csv() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let api_key = insert_test_api_key(&pool, "export-admin", "Admin").await;
        let _repo = insert_test_repo(&pool, "org/repo").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let events_payload = serde_json::json!({
            "events": [{
                "event_uuid": "export-drift-0001",
                "event_type": "commit",
                "user_login": "export-admin",
                "files": [],
                "status": "success",
                "branch": "main",
                "timestamp": 1700000010
            }],
            "client_version": "integration-test"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/events",
            Some(&events_payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "event ingest failed: {}", body);

        let drift_payload = serde_json::json!({
            "action": "drift_snapshot",
            "repo_name": "org/repo",
            "result": "observed",
            "metadata": {
                "drift_count": 2,
                "critical_count": 1
            }
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/drift-events",
            Some(&drift_payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "drift ingest failed: {}", body);

        let policy_request_payload = serde_json::json!({
            "config": {
                "branches": { "protected": ["main"], "patterns": ["feat/*"] },
                "rules": { "require_pull_request": true, "require_linked_ticket": true },
                "enforcement": { "pull_requests": "warn", "commits": "warn", "branches": "warn", "traceability": "warn" }
            },
            "reason": "Export coverage for policy requests"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/org/repo/requests",
            Some(&policy_request_payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "policy request ingest failed: {}",
            body
        );

        let json_export_payload = serde_json::json!({
            "export_type": "events"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/export",
            Some(&json_export_payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "json export failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let data = parsed["data"].as_object().expect("data object");
        assert!(
            data.get("events")
                .and_then(|v| v.as_array())
                .map(|v| !v.is_empty())
                .unwrap_or(false),
            "expected exported events array"
        );
        assert_eq!(
            data.get("policy_drift_events")
                .and_then(|v| v.as_array())
                .map(|v| v.len())
                .unwrap_or(0),
            1
        );
        assert_eq!(
            data.get("policy_change_requests")
                .and_then(|v| v.as_array())
                .map(|v| v.len())
                .unwrap_or(0),
            1
        );

        let csv_export_payload = serde_json::json!({
            "export_type": "events_csv"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/export",
            Some(&csv_export_payload.to_string()),
            Some(&api_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "csv export failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let csv_data = parsed["data"].as_str().unwrap_or_default();
        assert!(csv_data.contains("record_kind,id,source,event_type"));
        assert!(csv_data.contains("policy_drift"));
        assert!(csv_data.contains("policy_change_request"));
        assert!(csv_data.contains("org/repo"));

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_change_request_can_be_created_and_approved_by_admin() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let dev_key = insert_test_api_key(&pool, "policy-dev", "Developer").await;
        let _repo = insert_test_repo(&pool, "acme/repo").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let create_payload = serde_json::json!({
            "config": {
                "branches": { "protected": ["main"], "patterns": ["feat/*"] },
                "rules": { "require_pull_request": true, "min_approvals": 1 },
                "enforcement": { "pull_requests": "warn", "commits": "off", "branches": "warn", "traceability": "off" }
            },
            "reason": "Enable baseline protection"
        });

        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/acme/repo/requests",
            Some(&create_payload.to_string()),
            Some(&dev_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create request failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["accepted"], true);
        assert_eq!(parsed["status"], "pending");
        let request_id = parsed["request_id"].as_str().unwrap().to_string();

        let (status, _) = json_request(
            &app,
            "POST",
            &format!("/policy/requests/{}/approve", request_id),
            Some(&serde_json::json!({}).to_string()),
            Some(&dev_key),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, body) = json_request(
            &app,
            "POST",
            &format!("/policy/requests/{}/approve", request_id),
            Some(&serde_json::json!({"note":"Looks good"}).to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "approve request failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["status"], "approved");
        assert_eq!(parsed["decided_by"], "policy-admin");

        let (status, body) =
            json_request(&app, "GET", "/policy/acme/repo", None, Some(&admin_key)).await;
        assert_eq!(status, StatusCode::OK, "get policy failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["config"]["rules"]["require_pull_request"], true);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_change_request_rejects_self_approval() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let _repo = insert_test_repo(&pool, "acme/repo").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let create_payload = serde_json::json!({
            "config": { "rules": { "require_linked_ticket": true } },
            "reason": "Require ticket linkage"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/acme/repo/requests",
            Some(&create_payload.to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create request failed: {}", body);
        let request_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["request_id"]
            .as_str()
            .unwrap()
            .to_string();

        let (status, _) = json_request(
            &app,
            "POST",
            &format!("/policy/requests/{}/approve", request_id),
            Some(&serde_json::json!({"note":"approve own request"}).to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_change_request_can_be_rejected_by_admin() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let dev_key = insert_test_api_key(&pool, "policy-dev", "Developer").await;
        let _repo = insert_test_repo(&pool, "acme/repo").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let create_payload = serde_json::json!({
            "config": {
                "branches": { "protected": ["main"], "patterns": ["feat/*"] },
                "rules": { "require_linked_ticket": true, "require_pull_request": true },
                "enforcement": { "pull_requests": "warn", "commits": "warn", "branches": "warn", "traceability": "warn" }
            },
            "reason": "Request stricter traceability"
        });

        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/acme/repo/requests",
            Some(&create_payload.to_string()),
            Some(&dev_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create request failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["accepted"], true);
        let request_id = parsed["request_id"].as_str().unwrap().to_string();

        let reject_note = "Needs alignment with release policy";
        let (status, body) = json_request(
            &app,
            "POST",
            &format!("/policy/requests/{}/reject", request_id),
            Some(&serde_json::json!({"note": reject_note}).to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "reject request failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["status"], "rejected");
        assert_eq!(parsed["decided_by"], "policy-admin");
        assert_eq!(parsed["decision_note"], reject_note);

        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/acme/repo/requests?status=rejected&limit=10&offset=0",
            None,
            Some(&admin_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "list rejected requests failed: {}",
            body
        );
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed["total"].as_i64().unwrap_or_default() >= 1);
        let requests = parsed["requests"].as_array().cloned().unwrap_or_default();
        assert!(
            requests
                .iter()
                .any(|item| item["id"] == request_id && item["status"] == "rejected"),
            "expected rejected request id in filtered list"
        );

        let (status, _) = json_request(
            &app,
            "POST",
            &format!("/policy/requests/{}/approve", request_id),
            Some(&serde_json::json!({"note":"should conflict"}).to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);

        teardown(&admin_pool, &schema).await;
    }

    #[tokio::test]
    async fn policy_change_request_scope_is_enforced_for_multisession_listing() {
        let (pool, schema, admin_pool) = setup_or_skip!();
        let admin_key = insert_test_api_key(&pool, "policy-admin", "Admin").await;
        let dev_a_key = insert_test_api_key(&pool, "policy-dev-a", "Developer").await;
        let dev_b_key = insert_test_api_key(&pool, "policy-dev-b", "Developer").await;
        let _repo = insert_test_repo(&pool, "acme/repo").await;
        let db = Arc::new(Database::from_pool(pool.clone()));
        let app = build_test_app(db);

        let create_payload_a = serde_json::json!({
            "config": {
                "branches": { "protected": ["main"], "patterns": ["feat/*"] },
                "rules": { "require_pull_request": true, "min_approvals": 1 },
                "enforcement": { "pull_requests": "warn", "commits": "off", "branches": "warn", "traceability": "off" }
            },
            "reason": "Developer A proposal"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/acme/repo/requests",
            Some(&create_payload_a.to_string()),
            Some(&dev_a_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create request A failed: {}", body);
        let request_a_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["request_id"]
            .as_str()
            .unwrap()
            .to_string();

        let create_payload_b = serde_json::json!({
            "config": {
                "branches": { "protected": ["main"], "patterns": ["fix/*"] },
                "rules": { "require_linked_ticket": true },
                "enforcement": { "pull_requests": "off", "commits": "warn", "branches": "off", "traceability": "warn" }
            },
            "reason": "Developer B proposal"
        });
        let (status, body) = json_request(
            &app,
            "POST",
            "/policy/acme/repo/requests",
            Some(&create_payload_b.to_string()),
            Some(&dev_b_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create request B failed: {}", body);
        let request_b_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["request_id"]
            .as_str()
            .unwrap()
            .to_string();

        // Developer A only sees own requests.
        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/acme/repo/requests?limit=20&offset=0",
            None,
            Some(&dev_a_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "dev A list failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["total"], 1);
        let requests = parsed["requests"].as_array().cloned().unwrap_or_default();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["id"], request_a_id);
        assert_eq!(requests[0]["requested_by"], "policy-dev-a");

        // Developer B only sees own requests.
        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/acme/repo/requests?limit=20&offset=0",
            None,
            Some(&dev_b_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "dev B list failed: {}", body);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["total"], 1);
        let requests = parsed["requests"].as_array().cloned().unwrap_or_default();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["id"], request_b_id);
        assert_eq!(requests[0]["requested_by"], "policy-dev-b");

        // Admin sees all pending requests.
        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/acme/repo/requests?status=pending&limit=20&offset=0",
            None,
            Some(&admin_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "admin list pending failed: {}",
            body
        );
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed["total"].as_i64().unwrap_or_default() >= 2);
        let requests = parsed["requests"].as_array().cloned().unwrap_or_default();
        assert!(
            requests.iter().any(|item| item["id"] == request_a_id),
            "expected request A in admin list"
        );
        assert!(
            requests.iter().any(|item| item["id"] == request_b_id),
            "expected request B in admin list"
        );

        // After admin approval of request A, developer B cannot see it under approved filter.
        let (status, body) = json_request(
            &app,
            "POST",
            &format!("/policy/requests/{}/approve", request_a_id),
            Some(&serde_json::json!({"note":"scope check approval"}).to_string()),
            Some(&admin_key),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "approve request A failed: {}", body);

        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/acme/repo/requests?status=approved&limit=20&offset=0",
            None,
            Some(&dev_b_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "dev B approved list failed: {}",
            body
        );
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            parsed["total"].as_i64().unwrap_or_default(),
            0,
            "developer B should not see approvals from developer A"
        );

        let (status, body) = json_request(
            &app,
            "GET",
            "/policy/acme/repo/requests?status=approved&limit=20&offset=0",
            None,
            Some(&admin_key),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "admin approved list failed: {}",
            body
        );
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let requests = parsed["requests"].as_array().cloned().unwrap_or_default();
        assert!(
            requests.iter().any(|item| item["id"] == request_a_id),
            "expected approved request A in admin approved list"
        );

        teardown(&admin_pool, &schema).await;
    }
}
