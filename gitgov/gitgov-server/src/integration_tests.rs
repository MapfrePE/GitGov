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
    use crate::handlers::{self, AppState, ConversationalRuntime};
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                github_id BIGINT UNIQUE,
                login TEXT UNIQUE NOT NULL,
                name TEXT,
                avatar_url TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS repos (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                github_id BIGINT UNIQUE,
                full_name TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                private BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS api_keys (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                key_hash TEXT UNIQUE NOT NULL,
                client_id TEXT NOT NULL,
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                role TEXT NOT NULL DEFAULT 'Developer',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                last_used TIMESTAMPTZ,
                is_active BOOLEAN DEFAULT TRUE
            );

            CREATE TABLE IF NOT EXISTS client_events (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,
                repo_id UUID REFERENCES repos(id) ON DELETE CASCADE UNIQUE,
                config JSONB NOT NULL,
                checksum TEXT NOT NULL,
                override_actor TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS webhook_events (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                delivery_id TEXT UNIQUE NOT NULL,
                event_type TEXT NOT NULL,
                payload JSONB NOT NULL,
                processed BOOLEAN DEFAULT FALSE,
                error TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS pipeline_events (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS project_tickets (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                commit_sha TEXT NOT NULL,
                ticket_id TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                UNIQUE(commit_sha, ticket_id)
            );

            CREATE TABLE IF NOT EXISTS export_logs (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                org_id UUID,
                requested_by TEXT NOT NULL,
                format TEXT NOT NULL,
                filters JSONB DEFAULT '{}',
                event_count INTEGER DEFAULT 0,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS governance_events (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                org_id UUID,
                event_type TEXT NOT NULL,
                actor TEXT,
                repo TEXT,
                branch TEXT,
                details JSONB DEFAULT '{}',
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS pr_merges (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT,
                details JSONB DEFAULT '{}',
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS client_sessions (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                primary_login TEXT NOT NULL,
                alias_login TEXT UNIQUE NOT NULL,
                created_by TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS noncompliance_signals (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                violation_id UUID NOT NULL,
                actor TEXT NOT NULL,
                decision TEXT NOT NULL,
                reason TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS policy_history (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                repo_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                config JSONB,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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

    /// Build a minimal Router with auth middleware for integration testing.
    fn build_test_app(db: Arc<Database>) -> Router {
        let state = AppState {
            db: Arc::clone(&db),
            github_webhook_secret: None,
            github_personal_access_token: None,
            jenkins_webhook_secret: None,
            jira_webhook_secret: None,
            start_time: Instant::now(),
            worker_id: "test-worker".to_string(),
            http_client: reqwest::Client::new(),
            alert_webhook_url: None,
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
            logs_cache_ttl: Duration::from_millis(100),
            logs_cache_stale_on_error: Duration::from_millis(1000),
            logs_reject_offset_pagination: false,
            outbox_server_lease_enabled: false,
            outbox_server_lease_ttl_ms: 2000,
            outbox_lease_telemetry: Arc::new(Mutex::new(handlers::OutboxLeaseTelemetry::default())),
            logs_cache: Arc::new(Mutex::new(HashMap::new())),
            sse_tx: tokio::sync::broadcast::channel::<handlers::SseNotification>(64).0,
            sse_max_connections: Arc::new(Semaphore::new(50)),
        };

        let auth_routes = Router::new()
            .route("/events", post(handlers::ingest_client_events))
            .route("/logs", get(handlers::get_logs))
            .route("/stats", get(handlers::get_stats))
            .route("/stats/daily", get(handlers::get_daily_activity))
            .route("/dashboard", get(handlers::get_dashboard))
            .route("/me", get(handlers::get_me))
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
}
