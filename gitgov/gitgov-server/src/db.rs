use crate::models::*;
use sha2::Digest;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Duplicate: {0}")]
    Duplicate(String),
}

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, DbError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(Self { pool })
    }

    // ========================================================================
    // ORGANIZATIONS
    // ========================================================================

    pub async fn upsert_org(&self, github_id: i64, login: &str, name: Option<&str>, avatar_url: Option<&str>) -> Result<String, DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO orgs (github_id, login, name, avatar_url)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (github_id) DO UPDATE SET
                name = COALESCE($3, orgs.name),
                avatar_url = COALESCE($4, orgs.avatar_url),
                updated_at = NOW()
            RETURNING id::text
            "#,
        )
        .bind(github_id)
        .bind(login)
        .bind(name)
        .bind(avatar_url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.get("id"))
    }

    pub async fn get_org_by_login(&self, login: &str) -> Result<Option<Org>, DbError> {
        let result = sqlx::query(
            "SELECT id::text, github_id, login, name, avatar_url, created_at FROM orgs WHERE login = $1"
        )
        .bind(login)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                Ok(Some(Org {
                    id: row.get("id"),
                    github_id: row.get("github_id"),
                    login: row.get("login"),
                    name: row.get("name"),
                    avatar_url: row.get("avatar_url"),
                    created_at: created_at.timestamp_millis(),
                }))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // REPOSITORIES
    // ========================================================================

    pub async fn upsert_repo(
        &self,
        org_id: Option<&str>,
        github_id: i64,
        full_name: &str,
        name: &str,
        private: bool,
    ) -> Result<String, DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO repos (org_id, github_id, full_name, name, private)
            VALUES ($1::uuid, $2, $3, $4, $5)
            ON CONFLICT (full_name) DO UPDATE SET
                name = $4,
                private = $5,
                updated_at = NOW()
            RETURNING id::text
            "#,
        )
        .bind(org_id)
        .bind(github_id)
        .bind(full_name)
        .bind(name)
        .bind(private)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.get("id"))
    }

    pub async fn get_repo_by_full_name(&self, full_name: &str) -> Result<Option<Repo>, DbError> {
        let result = sqlx::query(
            "SELECT id::text, org_id::text, github_id, full_name, name, private, created_at FROM repos WHERE full_name = $1"
        )
        .bind(full_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                Ok(Some(Repo {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    github_id: row.get("github_id"),
                    full_name: row.get("full_name"),
                    name: row.get("name"),
                    private: row.get("private"),
                    created_at: created_at.timestamp_millis(),
                }))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // GITHUB EVENTS (Source of Truth)
    // ========================================================================

    pub async fn insert_github_event(&self, event: &GitHubEvent) -> Result<(), DbError> {
        let commit_shas_json = serde_json::to_string(&event.commit_shas)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        let result = sqlx::query(
            r#"
            INSERT INTO github_events (
                id, org_id, repo_id, delivery_id, event_type, actor_login, actor_id,
                ref_name, ref_type, before_sha, after_sha, commit_shas, commits_count, payload
            )
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13, $14::jsonb)
            ON CONFLICT (delivery_id) DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.org_id)
        .bind(&event.repo_id)
        .bind(&event.delivery_id)
        .bind(&event.event_type)
        .bind(&event.actor_login)
        .bind(&event.actor_id)
        .bind(&event.ref_name)
        .bind(&event.ref_type)
        .bind(&event.before_sha)
        .bind(&event.after_sha)
        .bind(&commit_shas_json)
        .bind(event.commits_count)
        .bind(&event.payload)
        .execute(&self.pool)
        .await;

        match result {
            Ok(res) if res.rows_affected() == 0 => {
                Err(DbError::Duplicate(format!("delivery_id: {}", event.delivery_id)))
            }
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => {
                Err(DbError::Duplicate(format!("delivery_id: {}", event.delivery_id)))
            }
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn get_github_events(&self, filter: &EventFilter) -> Result<Vec<GitHubEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
        let offset = filter.offset as i64;

        let mut query = String::from(
            "SELECT id::text, org_id::text, repo_id::text, delivery_id, event_type, actor_login, actor_id, ref_name, ref_type, before_sha, after_sha, commit_shas::text, commits_count, payload::text, created_at FROM github_events WHERE 1=1"
        );
        let mut param_count = 1;

        let mut conditions = Vec::new();

        if filter.start_date.is_some() {
            conditions.push(format!("created_at >= to_timestamp(${0}/1000.0)", param_count));
            param_count += 1;
        }
        if filter.end_date.is_some() {
            conditions.push(format!("created_at <= to_timestamp(${0}/1000.0)", param_count));
            param_count += 1;
        }
        if filter.user_login.is_some() {
            conditions.push(format!("actor_login = ${}", param_count));
            param_count += 1;
        }
        if filter.event_type.is_some() {
            conditions.push(format!("event_type = ${}", param_count));
            param_count += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET {}", param_count, param_count + 1));

        let mut sql_query = sqlx::query(&query);

        if let Some(start) = filter.start_date {
            sql_query = sql_query.bind(start);
        }
        if let Some(end) = filter.end_date {
            sql_query = sql_query.bind(end);
        }
        if let Some(ref login) = filter.user_login {
            sql_query = sql_query.bind(login);
        }
        if let Some(ref event_type) = filter.event_type {
            sql_query = sql_query.bind(event_type);
        }

        sql_query = sql_query.bind(limit).bind(offset);

        let rows = sql_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let events: Vec<GitHubEvent> = rows
            .iter()
            .map(|row| {
                let commit_shas_json: String = row.get("commit_shas");
                let payload_json: String = row.get("payload");
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");

                GitHubEvent {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    delivery_id: row.get("delivery_id"),
                    event_type: row.get("event_type"),
                    actor_login: row.get("actor_login"),
                    actor_id: row.get("actor_id"),
                    ref_name: row.get("ref_name"),
                    ref_type: row.get("ref_type"),
                    before_sha: row.get("before_sha"),
                    after_sha: row.get("after_sha"),
                    commit_shas: serde_json::from_str(&commit_shas_json).unwrap_or_default(),
                    commits_count: row.get("commits_count"),
                    payload: serde_json::from_str(&payload_json).unwrap_or(serde_json::Value::Null),
                    created_at: created_at.timestamp_millis(),
                }
            })
            .collect();

        Ok(events)
    }

    // ========================================================================
    // CLIENT EVENTS (Telemetry)
    // ========================================================================

    pub async fn insert_client_event(&self, event: &ClientEvent) -> Result<(), DbError> {
        let files_json = serde_json::to_string(&event.files)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        let result = sqlx::query(
            r#"
            INSERT INTO client_events (
                id, org_id, repo_id, event_uuid, event_type, user_login, user_name,
                branch, commit_sha, files, status, reason, metadata, client_version
            )
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10::jsonb, $11, $12, $13::jsonb, $14)
            ON CONFLICT (event_uuid) DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.org_id)
        .bind(&event.repo_id)
        .bind(&event.event_uuid)
        .bind(event.event_type.as_str())
        .bind(&event.user_login)
        .bind(&event.user_name)
        .bind(&event.branch)
        .bind(&event.commit_sha)
        .bind(&files_json)
        .bind(event.status.as_str())
        .bind(&event.reason)
        .bind(&event.metadata)
        .bind(&event.client_version)
        .execute(&self.pool)
        .await;

        match result {
            Ok(res) if res.rows_affected() == 0 => {
                Err(DbError::Duplicate(format!("event_uuid: {}", event.event_uuid)))
            }
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => {
                Err(DbError::Duplicate(format!("event_uuid: {}", event.event_uuid)))
            }
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn insert_client_events_batch(&self, events: &[ClientEvent]) -> Result<ClientEventResponse, DbError> {
        let mut accepted = Vec::new();
        let mut duplicates = Vec::new();
        let mut errors = Vec::new();

        for event in events {
            match self.insert_client_event(event).await {
                Ok(()) => accepted.push(event.event_uuid.clone()),
                Err(DbError::Duplicate(_)) => duplicates.push(event.event_uuid.clone()),
                Err(e) => errors.push(EventError {
                    event_uuid: event.event_uuid.clone(),
                    error: e.to_string(),
                }),
            }
        }

        Ok(ClientEventResponse {
            accepted,
            duplicates,
            errors,
        })
    }

    pub async fn get_client_events(&self, filter: &EventFilter) -> Result<Vec<ClientEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
        let offset = filter.offset as i64;

        let mut query = String::from(
            "SELECT id::text, org_id::text, repo_id::text, event_uuid, event_type, user_login, user_name, branch, commit_sha, files::text, status, reason, metadata::text, client_version, created_at FROM client_events WHERE 1=1"
        );
        let mut param_count = 1;

        let mut conditions = Vec::new();

        if filter.start_date.is_some() {
            conditions.push(format!("created_at >= to_timestamp(${0}/1000.0)", param_count));
            param_count += 1;
        }
        if filter.end_date.is_some() {
            conditions.push(format!("created_at <= to_timestamp(${0}/1000.0)", param_count));
            param_count += 1;
        }
        if filter.user_login.is_some() {
            conditions.push(format!("user_login = ${}", param_count));
            param_count += 1;
        }
        if filter.event_type.is_some() {
            conditions.push(format!("event_type = ${}", param_count));
            param_count += 1;
        }
        if filter.status.is_some() {
            conditions.push(format!("status = ${}", param_count));
            param_count += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET {}", param_count, param_count + 1));

        let mut sql_query = sqlx::query(&query);

        if let Some(start) = filter.start_date {
            sql_query = sql_query.bind(start);
        }
        if let Some(end) = filter.end_date {
            sql_query = sql_query.bind(end);
        }
        if let Some(ref login) = filter.user_login {
            sql_query = sql_query.bind(login);
        }
        if let Some(ref event_type) = filter.event_type {
            sql_query = sql_query.bind(event_type);
        }
        if let Some(ref status) = filter.status {
            sql_query = sql_query.bind(status);
        }

        sql_query = sql_query.bind(limit).bind(offset);

        let rows = sql_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let events: Vec<ClientEvent> = rows
            .iter()
            .map(|row| {
                let files_json: String = row.get("files");
                let metadata_json: String = row.get("metadata");
                let event_type_str: String = row.get("event_type");
                let status_str: String = row.get("status");
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");

                ClientEvent {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    event_uuid: row.get("event_uuid"),
                    event_type: ClientEventType::from_str(&event_type_str),
                    user_login: row.get("user_login"),
                    user_name: row.get("user_name"),
                    branch: row.get("branch"),
                    commit_sha: row.get("commit_sha"),
                    files: serde_json::from_str(&files_json).unwrap_or_default(),
                    status: EventStatus::from_str(&status_str),
                    reason: row.get("reason"),
                    metadata: serde_json::from_str(&metadata_json).unwrap_or(serde_json::Value::Null),
                    client_version: row.get("client_version"),
                    created_at: created_at.timestamp_millis(),
                }
            })
            .collect();

        Ok(events)
    }

    // ========================================================================
    // COMBINED EVENTS (for dashboard)
    // ========================================================================

    pub async fn get_combined_events(&self, filter: &EventFilter) -> Result<Vec<CombinedEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i32;
        let offset = filter.offset as i32;

        let result = sqlx::query(
            "SELECT * FROM get_combined_events($1, $2, NULL, NULL, $3, $4, $5)"
        )
        .bind(limit)
        .bind(offset)
        .bind(&filter.source)
        .bind(&filter.event_type)
        .bind(&filter.user_login)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let events: Vec<CombinedEvent> = result
            .iter()
            .map(|row| {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                let details_json: serde_json::Value = row.get("details");

                CombinedEvent {
                    id: row.get("id"),
                    source: row.get("source"),
                    event_type: row.get("event_type"),
                    created_at: created_at.timestamp_millis(),
                    user_login: row.get("user_login"),
                    repo_name: row.get("repo_name"),
                    branch: row.get("branch"),
                    status: row.get("status"),
                    details: details_json,
                }
            })
            .collect();

        Ok(events)
    }

    // ========================================================================
    // STATS
    // ========================================================================

    pub async fn get_stats(&self) -> Result<AuditStats, DbError> {
        let row = sqlx::query("SELECT get_audit_stats() as stats")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let stats_json: sqlx::types::Json<AuditStats> = row.get("stats");

        Ok(stats_json.0)
    }

    // ========================================================================
    // POLICIES
    // ========================================================================

    pub async fn save_policy(&self, repo_id: &str, config: &GitGovConfig, checksum: &str, override_actor: &str) -> Result<(), DbError> {
        let config_json = serde_json::to_value(config)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO policies (repo_id, config, checksum, override_actor, updated_at)
            VALUES ($1::uuid, $2, $3, $4, NOW())
            ON CONFLICT (repo_id) DO UPDATE SET 
                config = $2, 
                checksum = $3, 
                override_actor = $4,
                updated_at = NOW()
            "#,
        )
        .bind(repo_id)
        .bind(&config_json)
        .bind(checksum)
        .bind(override_actor)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_policy(&self, repo_id: &str) -> Result<Option<PolicyResponse>, DbError> {
        let result = sqlx::query(
            r#"
            SELECT config, checksum, updated_at 
            FROM policies 
            WHERE repo_id = $1::uuid
            "#,
        )
        .bind(repo_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => {
                let config: serde_json::Value = row.get("config");
                let config: GitGovConfig = serde_json::from_value(config.clone())
                    .map_err(|e| DbError::SerializationError(e.to_string()))?;
                let checksum: String = row.get("checksum");
                let updated_at: chrono::DateTime<chrono::Utc> = row.get("updated_at");

                Ok(Some(PolicyResponse {
                    version: "1.0".to_string(),
                    checksum,
                    config,
                    updated_at: updated_at.timestamp_millis(),
                }))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // WEBHOOK EVENTS (raw storage for debugging)
    // ========================================================================

    pub async fn store_webhook_event(
        &self,
        delivery_id: &str,
        event_type: &str,
        signature: Option<&str>,
        payload: &serde_json::Value,
    ) -> Result<String, DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO webhook_events (delivery_id, event_type, signature, payload)
            VALUES ($1, $2, $3, $4)
            RETURNING id::text
            "#,
        )
        .bind(delivery_id)
        .bind(event_type)
        .bind(signature)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.get("id"))
    }

    pub async fn mark_webhook_processed(&self, id: &str, error: Option<&str>) -> Result<(), DbError> {
        sqlx::query(
            r#"
            UPDATE webhook_events 
            SET processed = TRUE, processed_at = NOW(), error = $2
            WHERE id = $1::uuid
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    // ========================================================================
    // API KEYS
    // ========================================================================

    pub async fn validate_api_key(&self, key_hash: &str) -> Result<Option<(String, UserRole, Option<String>)>, DbError> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys SET last_used = NOW() WHERE key_hash = $1 AND is_active = TRUE
            RETURNING client_id, role, org_id::text
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => {
                let role: String = row.get("role");
                let role = UserRole::from_str(&role);
                let client_id: String = row.get("client_id");
                let org_id: Option<String> = row.get("org_id");
                Ok(Some((client_id, role, org_id)))
            }
            None => Ok(None),
        }
    }

    pub async fn create_api_key(&self, key_hash: &str, client_id: &str, org_id: Option<&str>, role: &UserRole) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO api_keys (key_hash, client_id, org_id, role) VALUES ($1, $2, $3::uuid, $4)
            "#,
        )
        .bind(key_hash)
        .bind(client_id)
        .bind(org_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn count_api_keys(&self) -> Result<i64, DbError> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM api_keys WHERE is_active = TRUE")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = result.get("count");
        Ok(count)
    }

    // ========================================================================
    // BOOTSTRAP
    // ========================================================================

    pub async fn bootstrap_admin_key(&self) -> Result<Option<String>, DbError> {
        let count = self.count_api_keys().await?;
        
        if count > 0 {
            return Ok(None);
        }

        let api_key = uuid::Uuid::new_v4().to_string();
        let key_hash = format!("{:x}", sha2::Sha256::digest(api_key.as_bytes()));
        let client_id = "bootstrap-admin";

        self.create_api_key(&key_hash, client_id, None, &UserRole::Admin).await?;

        Ok(Some(api_key))
    }

    // ========================================================================
    // HEALTH CHECK
    // ========================================================================

    pub async fn health_check(&self) -> Result<(bool, i64), DbError> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM client_events WHERE status = 'pending'")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = result.get("count");
        Ok((true, count))
    }

    // ========================================================================
    // NONCOMPLIANCE SIGNALS
    // ========================================================================

    pub async fn get_noncompliance_signals(
        &self,
        org_name: Option<&str>,
        confidence: Option<&str>,
        status: Option<&str>,
        signal_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<NoncomplianceSignal>, i64), DbError> {
        let mut conditions = Vec::new();
        let mut param_count = 1;

        let org_id_subquery = if org_name.is_some() {
            conditions.push(format!("ns.org_id = (SELECT id FROM orgs WHERE login = ${})", param_count));
            param_count += 1;
            true
        } else {
            false
        };

        if confidence.is_some() {
            conditions.push(format!("ns.confidence = ${}", param_count));
            param_count += 1;
        }
        if status.is_some() {
            conditions.push(format!("ns.status = ${}", param_count));
            param_count += 1;
        }
        if signal_type.is_some() {
            conditions.push(format!("ns.signal_type = ${}", param_count));
            param_count += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let count_query = format!("SELECT COUNT(*) as total FROM noncompliance_signals ns{}", where_clause);
        
        let mut count_sql = sqlx::query(&count_query);
        let mut param_idx = 1;
        
        if let Some(org) = org_name {
            count_sql = count_sql.bind(org);
            param_idx += 1;
        }
        if let Some(c) = confidence {
            count_sql = count_sql.bind(c);
            param_idx += 1;
        }
        if let Some(s) = status {
            count_sql = count_sql.bind(s);
            param_idx += 1;
        }
        if let Some(st) = signal_type {
            count_sql = count_sql.bind(st);
        }

        let count_row = count_sql
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let total: i64 = count_row.get("total");

        let data_query = format!(
            "SELECT ns.id::text, ns.org_id::text, ns.repo_id::text, ns.github_event_id::text, ns.client_event_id::text, \
             ns.signal_type, ns.confidence, ns.actor_login, ns.branch, ns.commit_sha, ns.evidence, ns.context, \
             ns.status, ns.investigated_by, ns.investigated_at, ns.investigation_notes, ns.created_at \
             FROM noncompliance_signals ns{} ORDER BY ns.created_at DESC LIMIT ${} OFFSET ${}",
            where_clause, param_count, param_count + 1
        );

        let mut data_sql = sqlx::query(&data_query);
        
        if let Some(org) = org_name {
            data_sql = data_sql.bind(org);
        }
        if let Some(c) = confidence {
            data_sql = data_sql.bind(c);
        }
        if let Some(s) = status {
            data_sql = data_sql.bind(s);
        }
        if let Some(st) = signal_type {
            data_sql = data_sql.bind(st);
        }
        data_sql = data_sql.bind(limit).bind(offset);

        let rows = data_sql
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let signals: Vec<NoncomplianceSignal> = rows
            .iter()
            .map(|row| {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                let investigated_at: Option<chrono::DateTime<chrono::Utc>> = row.get("investigated_at");

                NoncomplianceSignal {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    github_event_id: row.get("github_event_id"),
                    client_event_id: row.get("client_event_id"),
                    signal_type: row.get("signal_type"),
                    confidence: row.get("confidence"),
                    actor_login: row.get("actor_login"),
                    branch: row.get("branch"),
                    commit_sha: row.get("commit_sha"),
                    evidence: row.get("evidence"),
                    context: row.get("context"),
                    status: row.get("status"),
                    investigated_by: row.get("investigated_by"),
                    investigated_at: investigated_at.map(|t| t.timestamp_millis()),
                    investigation_notes: row.get("investigation_notes"),
                    created_at: created_at.timestamp_millis(),
                }
            })
            .collect();

        Ok((signals, total))
    }

    pub async fn update_signal_status(
        &self,
        signal_id: &str,
        status: &str,
        notes: Option<&str>,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            UPDATE noncompliance_signals 
            SET status = $2, 
                investigation_notes = $3,
                investigated_at = NOW()
            WHERE id = $1::uuid
            "#,
        )
        .bind(signal_id)
        .bind(status)
        .bind(notes)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_signal_by_id(&self, signal_id: &str) -> Result<Option<NoncomplianceSignal>, DbError> {
        let result = sqlx::query(
            r#"
            SELECT id::text, org_id::text, repo_id::text, github_event_id::text, client_event_id::text,
                   signal_type, confidence, actor_login, branch, commit_sha, evidence, context,
                   status, investigated_by, investigated_at, investigation_notes, created_at
            FROM noncompliance_signals WHERE id = $1::uuid
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                let investigated_at: Option<chrono::DateTime<chrono::Utc>> = row.get("investigated_at");
                
                Ok(Some(NoncomplianceSignal {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    github_event_id: row.get("github_event_id"),
                    client_event_id: row.get("client_event_id"),
                    signal_type: row.get("signal_type"),
                    confidence: row.get("confidence"),
                    actor_login: row.get("actor_login"),
                    branch: row.get("branch"),
                    commit_sha: row.get("commit_sha"),
                    evidence: row.get("evidence"),
                    context: row.get("context"),
                    status: row.get("status"),
                    investigated_by: row.get("investigated_by"),
                    investigated_at: investigated_at.map(|t| t.timestamp_millis()),
                    investigation_notes: row.get("investigation_notes"),
                    created_at: created_at.timestamp_millis(),
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn confirm_signal_as_violation(
        &self,
        signal_id: &str,
        confirmed_by: &str,
        severity: &str,
    ) -> Result<String, DbError> {
        let signal = self.get_signal_by_id(signal_id).await?
            .ok_or_else(|| DbError::NotFound(format!("Signal not found: {}", signal_id)))?;

        let violation_id = uuid::Uuid::new_v4().to_string();

        // Insert into violations - APPEND ONLY
        sqlx::query(
            r#"
            INSERT INTO violations (
                id, org_id, repo_id, github_event_id, client_event_id,
                violation_type, severity, confidence_level, reason,
                user_login, branch, commit_sha, details
            )
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&violation_id)
        .bind(&signal.org_id)
        .bind(&signal.repo_id)
        .bind(&signal.github_event_id)
        .bind(&signal.client_event_id)
        .bind(&signal.signal_type)
        .bind(severity)
        .bind(&signal.confidence)
        .bind(&signal.investigation_notes)
        .bind(&signal.actor_login)
        .bind(&signal.branch)
        .bind(&signal.commit_sha)
        .bind(&signal.evidence)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        // NOTE: We do NOT update noncompliance_signals - it's append-only.
        // The signal remains as-is, the violation is a new record.
        // To track confirmation workflow, use a separate signal_decisions table
        // or track via the violation's creation with confirmed_by.

        // Insert a signal_decision record for audit trail (if table exists)
        let _ = sqlx::query(
            r#"
            INSERT INTO signal_decisions (
                id, signal_id, decision, decided_by, severity, created_at
            )
            VALUES ($1::uuid, $2::uuid, 'confirmed', $3, $4, NOW())
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&uuid::Uuid::new_v4().to_string())
        .bind(signal_id)
        .bind(confirmed_by)
        .bind(severity)
        .execute(&self.pool)
        .await;

        tracing::info!(
            "Signal {} confirmed as violation {} by {}",
            signal_id,
            violation_id,
            confirmed_by
        );

        Ok(violation_id)
    }

    pub async fn detect_noncompliance_signals(&self, org_id: &str) -> Result<i64, DbError> {
        let result = sqlx::query(
            "SELECT detect_noncompliance_signals($1::uuid) as count"
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.get("count"))
    }

    // ========================================================================
    // COMPLIANCE DASHBOARD
    // ========================================================================

    pub async fn get_compliance_dashboard(&self, org_id: &str) -> Result<ComplianceDashboard, DbError> {
        let row = sqlx::query(
            "SELECT get_compliance_dashboard($1::uuid) as dashboard"
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let dashboard: sqlx::types::Json<ComplianceDashboard> = row.get("dashboard");

        Ok(dashboard.0)
    }

    // ========================================================================
    // POLICY HISTORY
    // ========================================================================

    pub async fn get_policy_history(&self, repo_id: &str) -> Result<Vec<PolicyHistory>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM get_policy_history($1::uuid)"
        )
        .bind(repo_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let history: Vec<PolicyHistory> = rows
            .iter()
            .map(|row| {
                let config: serde_json::Value = row.get("config");
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");

                PolicyHistory {
                    id: row.get("id"),
                    repo_id: repo_id.to_string(),
                    config: serde_json::from_value(config).unwrap_or_default(),
                    checksum: row.get("checksum"),
                    changed_by: row.get("changed_by"),
                    change_type: row.get("change_type"),
                    previous_checksum: row.get("previous_checksum"),
                    created_at: created_at.timestamp_millis(),
                }
            })
            .collect();

        Ok(history)
    }

    // ========================================================================
    // EXPORT LOGS
    // ========================================================================

    pub async fn create_export_log(&self, export: &ExportLog) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO export_logs (id, org_id, exported_by, export_type, date_range_start, date_range_end, filters, record_count, content_hash, file_path, created_at)
            VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, to_timestamp($11/1000.0))
            "#,
        )
        .bind(&export.id)
        .bind(&export.org_id)
        .bind(&export.exported_by)
        .bind(&export.export_type)
        .bind(export.date_range_start.map(|t| chrono::DateTime::from_timestamp_millis(t)))
        .bind(export.date_range_end.map(|t| chrono::DateTime::from_timestamp_millis(t)))
        .bind(&export.filters)
        .bind(export.record_count)
        .bind(&export.content_hash)
        .bind(&export.file_path)
        .bind(export.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    // ========================================================================
    // GOVERNANCE EVENTS (Audit Log Streaming)
    // ========================================================================

    pub async fn insert_governance_event(&self, event: &GovernanceEvent) -> Result<(), DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO governance_events (
                id, org_id, repo_id, delivery_id, event_type, actor_login, 
                target, old_value, new_value, payload
            )
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (delivery_id) DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.org_id)
        .bind(&event.repo_id)
        .bind(&event.delivery_id)
        .bind(&event.event_type)
        .bind(&event.actor_login)
        .bind(&event.target)
        .bind(&event.old_value)
        .bind(&event.new_value)
        .bind(&event.payload)
        .execute(&self.pool)
        .await;

        match result {
            Ok(res) if res.rows_affected() == 0 => {
                Err(DbError::Duplicate(format!("delivery_id: {}", event.delivery_id)))
            }
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => {
                Err(DbError::Duplicate(format!("delivery_id: {}", event.delivery_id)))
            }
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn insert_governance_events_batch(&self, events: &[GovernanceEvent]) -> Result<(i32, Vec<String>), DbError> {
        let mut accepted = 0;
        let mut errors = Vec::new();

        for event in events {
            match self.insert_governance_event(event).await {
                Ok(()) => accepted += 1,
                Err(DbError::Duplicate(_)) => {}
                Err(e) => errors.push(format!("{}: {}", event.delivery_id, e)),
            }
        }

        Ok((accepted, errors))
    }

    pub async fn get_governance_events(
        &self,
        org_id: Option<&str>,
        event_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<GovernanceEvent>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT id::text, org_id::text, repo_id::text, delivery_id, event_type, actor_login,
                   target, old_value, new_value, payload, created_at
            FROM governance_events
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
              AND ($2 IS NULL OR event_type = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(org_id)
        .bind(event_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let events: Vec<GovernanceEvent> = rows
            .iter()
            .map(|row| {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                GovernanceEvent {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    delivery_id: row.get("delivery_id"),
                    event_type: row.get("event_type"),
                    actor_login: row.get("actor_login"),
                    target: row.get("target"),
                    old_value: row.get("old_value"),
                    new_value: row.get("new_value"),
                    payload: row.get("payload"),
                    created_at: created_at.timestamp_millis(),
                }
            })
            .collect();

        Ok(events)
    }

    // ========================================================================
    // JOB QUEUE (Production-hardened with backpressure control)
    // ========================================================================
    // Features:
    // - Atomic claim with FOR UPDATE SKIP LOCKED (no race conditions)
    // - Dedupe: only 1 pending/running job per (org_id, job_type)
    // - Exponential backoff: 30s * 2^attempts, capped at 1 hour
    // - Dead-letter: jobs exceeding max_attempts marked as 'dead'
    // - Structured logging with job_id, org_id, duration_ms
    // - Safe stale reset with backoff scheduling

    /// Enqueue a job (idempotent - one pending/running job per org+type)
    /// Uses partial unique index to prevent duplicate jobs.
    /// FIX: On conflict, returns the existing job's id instead of a fake UUID.
    pub async fn enqueue_job(
        &self,
        org_id: &str,
        job_type: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<String, DbError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let payload = payload.unwrap_or(serde_json::Value::Null);
        let start = std::time::Instant::now();

        let result = sqlx::query(
            r#"
            INSERT INTO jobs (id, org_id, job_type, status, payload, max_attempts, created_at)
            VALUES ($1::uuid, $2::uuid, $3, 'pending', $4, 10, NOW())
            ON CONFLICT (org_id, job_type) WHERE status IN ('pending', 'running') DO NOTHING
            RETURNING id::text
            "#,
        )
        .bind(&job_id)
        .bind(org_id)
        .bind(job_type)
        .bind(&payload)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => {
                let returned_id: String = row.get("id");
                tracing::info!(
                    job_id = %returned_id,
                    org_id = %org_id,
                    job_type = %job_type,
                    duration_ms = start.elapsed().as_millis() as u64,
                    "Job enqueued"
                );
                Ok(returned_id)
            }
            None => {
                tracing::debug!(
                    org_id = %org_id,
                    job_type = %job_type,
                    "Job already pending/running, fetching existing id"
                );
                let existing = sqlx::query(
                    r#"
                    SELECT id::text FROM jobs 
                    WHERE org_id = $1::uuid 
                      AND job_type = $2 
                      AND status IN ('pending', 'running')
                    LIMIT 1
                    "#,
                )
                .bind(org_id)
                .bind(job_type)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| DbError::DatabaseError(e.to_string()))?;

                match existing {
                    Some(row) => Ok(row.get("id")),
                    None => {
                        tracing::warn!(
                            org_id = %org_id,
                            job_type = %job_type,
                            "Job not found after conflict - returning new id"
                        );
                        Ok(job_id)
                    }
                }
            }
        }
    }

    /// Claim next pending job atomically.
    /// Uses FOR UPDATE SKIP LOCKED to prevent race conditions.
    /// Records start time for duration tracking.
    pub async fn claim_job(&self, worker_id: &str) -> Result<Option<Job>, DbError> {
        let start = std::time::Instant::now();

        let row = sqlx::query(
            r#"
            UPDATE jobs 
            SET status = 'running', 
                locked_at = NOW(),
                locked_by = $1,
                attempts = attempts + 1,
                started_at = NOW()
            WHERE id = (
                SELECT id FROM jobs 
                WHERE status = 'pending' AND next_run_at <= NOW()
                ORDER BY priority DESC, created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id::text, org_id::text, job_type, status, priority, payload,
                      attempts, max_attempts, created_at, locked_at, started_at
            "#,
        )
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match row {
            Some(r) => {
                let job_id: String = r.get("id");
                let org_id: String = r.get("org_id");
                let job_type: String = r.get("job_type");
                let attempts: i32 = r.get("attempts");
                
                tracing::info!(
                    job_id = %job_id,
                    org_id = %org_id,
                    job_type = %job_type,
                    attempt = attempts,
                    worker_id = %worker_id,
                    claim_duration_ms = start.elapsed().as_millis() as u64,
                    "Job claimed"
                );

                let created_at: chrono::DateTime<chrono::Utc> = r.get("created_at");
                let locked_at: Option<chrono::DateTime<chrono::Utc>> = r.get("locked_at");
                let started_at: Option<chrono::DateTime<chrono::Utc>> = r.get("started_at");
                
                Ok(Some(Job {
                    id: job_id,
                    org_id,
                    job_type,
                    status: r.get("status"),
                    priority: r.get("priority"),
                    payload: r.get("payload"),
                    attempts,
                    max_attempts: r.get("max_attempts"),
                    created_at: created_at.timestamp_millis(),
                    locked_at: locked_at.map(|t| t.timestamp_millis()),
                    locked_by: Some(worker_id.to_string()),
                    started_at: started_at.map(|t| t.timestamp_millis()),
                    duration_ms: None,
                }))
            }
            None => Ok(None),
        }
    }

    /// Complete a job successfully.
    /// Records duration for metrics.
    pub async fn complete_job(&self, job_id: &str) -> Result<(), DbError> {
        let start = std::time::Instant::now();

        sqlx::query(
            r#"
            UPDATE jobs 
            SET status = 'completed', 
                completed_at = NOW(),
                duration_ms = (EXTRACT(EPOCH FROM (NOW() - started_at)) * 1000)::BIGINT
            WHERE id = $1::uuid
            "#,
        )
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tracing::info!(
            job_id = %job_id,
            total_duration_ms = start.elapsed().as_millis() as u64,
            "Job completed"
        );
        Ok(())
    }

    /// Fail a job with exponential backoff retry scheduling.
    /// If attempts >= max_attempts, marks as 'dead' (dead-letter queue).
    /// Backoff: 30s * 2^attempts, capped at 1 hour.
    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), DbError> {
        let start = std::time::Instant::now();

        // Calculate backoff using PostgreSQL function
        let row = sqlx::query(
            r#"
            UPDATE jobs 
            SET status = CASE 
                WHEN attempts >= max_attempts THEN 'dead' 
                ELSE 'pending' 
            END,
            last_error = $1,
            next_run_at = CASE 
                WHEN attempts < max_attempts THEN NOW() + (job_backoff_seconds(attempts) || ' seconds')::INTERVAL 
                ELSE NULL 
            END,
            locked_at = NULL,
            locked_by = NULL,
            duration_ms = (EXTRACT(EPOCH FROM (NOW() - started_at)) * 1000)::BIGINT
            WHERE id = $2::uuid
            RETURNING status, attempts, max_attempts
            "#,
        )
        .bind(error)
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match row {
            Some(r) => {
                let status: String = r.get("status");
                let attempts: i32 = r.get("attempts");
                let max_attempts: i32 = r.get("max_attempts");

                if status == "dead" {
                    tracing::warn!(
                        job_id = %job_id,
                        attempts = attempts,
                        max_attempts = max_attempts,
                        error = %error,
                        "Job dead (exceeded max attempts) - moved to dead-letter"
                    );
                } else {
                    let backoff_secs = Self::calculate_backoff(attempts);
                    tracing::warn!(
                        job_id = %job_id,
                        attempt = attempts,
                        max_attempts = max_attempts,
                        backoff_secs = backoff_secs,
                        error = %error,
                        "Job failed, scheduled retry"
                    );
                }
            }
            None => {
                tracing::error!(job_id = %job_id, "Job not found for failure update");
            }
        }

        let _ = start;
        Ok(())
    }

    /// Calculate exponential backoff in seconds.
    /// Formula: 30 * 2^attempts, capped at 3600 (1 hour).
    fn calculate_backoff(attempts: i32) -> u64 {
        let base: u64 = 30;
        let max: u64 = 3600;
        let backoff = base.saturating_mul(1u64 << attempts.min(7));
        backoff.min(max)
    }

    /// Safely reset stale jobs (locked > TTL minutes).
    /// Uses FOR UPDATE SKIP LOCKED to prevent race conditions.
    /// FIX: Uses attempts+1 for backoff, marks dead if max_attempts exceeded.
    pub async fn reset_stale_jobs(&self) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            WITH stale_jobs AS (
                SELECT id, attempts, max_attempts FROM jobs 
                WHERE status = 'running' 
                  AND locked_at < NOW() - INTERVAL '5 minutes'
                FOR UPDATE SKIP LOCKED
            )
            UPDATE jobs 
            SET status = CASE 
                WHEN (attempts + 1) >= max_attempts THEN 'dead'
                ELSE 'pending'
            END,
            locked_at = NULL,
            locked_by = NULL,
            started_at = NULL,
            attempts = attempts + 1,
            last_error = CASE 
                WHEN (attempts + 1) >= max_attempts THEN 'Job exceeded max_attempts after timeout'
                ELSE 'Job timed out after 5 minutes'
            END,
            next_run_at = CASE 
                WHEN (attempts + 1) < max_attempts THEN NOW() + (job_backoff_seconds(attempts + 1) || ' seconds')::INTERVAL
                ELSE NULL
            END,
            completed_at = CASE 
                WHEN (attempts + 1) >= max_attempts THEN NOW()
                ELSE completed_at
            END
            WHERE id IN (SELECT id FROM stale_jobs)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count = result.rows_affected() as i64;
        if count > 0 {
            tracing::warn!(
                stale_count = count,
                ttl_minutes = 5,
                "Reset stale jobs with backoff scheduling (dead-letter aware)"
            );
        }
        Ok(count)
    }

    /// Reset stale jobs using the SQL function (single source of truth).
    /// This calls reset_stale_jobs_safe() defined in supabase_schema_v2.sql.
    pub async fn reset_stale_jobs_safe(&self) -> Result<i64, DbError> {
        let result = sqlx::query("SELECT reset_stale_jobs_safe(5) as count")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = result.get("count");
        if count > 0 {
            tracing::warn!(
                stale_count = count,
                ttl_minutes = 5,
                "Reset stale jobs via SQL function"
            );
        }
        Ok(count)
    }

    /// Get job queue metrics for observability.
    pub async fn get_job_metrics(&self) -> Result<JobMetrics, DbError> {
        let row = sqlx::query("SELECT get_job_metrics() as metrics")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let metrics: sqlx::types::Json<JobMetrics> = row.get("metrics");
        Ok(metrics.0)
    }

    /// Execute detect_noncompliance_signals via job.
    /// This is idempotent - uses ingested_at cursor.
    pub async fn execute_detect_signals(&self, org_id: &str) -> Result<i64, DbError> {
        let start = std::time::Instant::now();
        
        let result = sqlx::query(
            "SELECT detect_noncompliance_signals($1::uuid) as count"
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = result.get("count");
        
        tracing::info!(
            org_id = %org_id,
            signals_created = count,
            duration_ms = start.elapsed().as_millis() as u64,
            "Signal detection completed"
        );

        Ok(count)
    }

    /// Get dead-letter jobs for inspection.
    pub async fn get_dead_jobs(&self, limit: i64) -> Result<Vec<Job>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT id::text, org_id::text, job_type, status, priority, payload,
                   attempts, max_attempts, last_error, created_at, locked_at
            FROM jobs 
            WHERE status = 'dead'
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let jobs: Vec<Job> = rows
            .iter()
            .map(|r| {
                let created_at: chrono::DateTime<chrono::Utc> = r.get("created_at");
                Job {
                    id: r.get("id"),
                    org_id: r.get("org_id"),
                    job_type: r.get("job_type"),
                    status: r.get("status"),
                    priority: r.get("priority"),
                    payload: r.get("payload"),
                    attempts: r.get("attempts"),
                    max_attempts: r.get("max_attempts"),
                    created_at: created_at.timestamp_millis(),
                    locked_at: None,
                    locked_by: None,
                    started_at: None,
                    duration_ms: None,
                }
            })
            .collect();

        Ok(jobs)
    }

    /// Retry a dead job (manual intervention).
    pub async fn retry_dead_job(&self, job_id: &str) -> Result<(), DbError> {
        sqlx::query(
            r#"
            UPDATE jobs 
            SET status = 'pending',
                attempts = 0,
                next_run_at = NOW(),
                last_error = NULL
            WHERE id = $1::uuid AND status = 'dead'
            "#,
        )
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tracing::info!(job_id = %job_id, "Dead job queued for retry");
        Ok(())
    }

    // ========================================================================
    // VIOLATION DECISIONS (v3 schema)
    // ========================================================================

    /// Add a decision to a violation (append-only audit trail).
    /// Decision types: acknowledged, false_positive, resolved, escalated, dismissed, wont_fix
    pub async fn add_violation_decision(
        &self,
        violation_id: &str,
        decision_type: &str,
        decided_by: &str,
        notes: Option<&str>,
        evidence: Option<serde_json::Value>,
    ) -> Result<String, DbError> {
        let result = sqlx::query(
            r#"
            SELECT add_violation_decision(
                $1::uuid,
                $2,
                $3,
                $4,
                $5
            ) as decision_id
            "#,
        )
        .bind(violation_id)
        .bind(decision_type)
        .bind(decided_by)
        .bind(notes)
        .bind(evidence.unwrap_or(serde_json::Value::Null))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let decision_id: String = result.get("decision_id");
        
        tracing::info!(
            violation_id = %violation_id,
            decision_type = %decision_type,
            decided_by = %decided_by,
            "Violation decision recorded"
        );
        
        Ok(decision_id)
    }

    /// Get decision history for a violation.
    pub async fn get_violation_decisions(
        &self,
        violation_id: &str,
    ) -> Result<Vec<ViolationDecision>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT id::text, violation_id::text, decision_type, decided_by,
                   decided_at, notes, evidence, created_at
            FROM violation_decisions
            WHERE violation_id = $1::uuid
            ORDER BY decided_at DESC
            "#,
        )
        .bind(violation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let decisions: Vec<ViolationDecision> = rows
            .iter()
            .map(|r| {
                let decided_at: chrono::DateTime<chrono::Utc> = r.get("decided_at");
                let created_at: chrono::DateTime<chrono::Utc> = r.get("created_at");
                ViolationDecision {
                    id: r.get("id"),
                    violation_id: r.get("violation_id"),
                    decision_type: r.get("decision_type"),
                    decided_by: r.get("decided_by"),
                    decided_at: decided_at.timestamp_millis(),
                    notes: r.get("notes"),
                    evidence: r.get("evidence"),
                    created_at: created_at.timestamp_millis(),
                }
            })
            .collect();

        Ok(decisions)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: String,
    pub org_id: String,
    pub job_type: String,
    pub status: String,
    pub priority: i32,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub max_attempts: i32,
    pub created_at: i64,
    pub locked_at: Option<i64>,
    pub locked_by: Option<String>,
    pub started_at: Option<i64>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct JobMetrics {
    pub pending: i64,
    pub running: i64,
    pub completed_today: i64,
    pub failed_today: i64,
    pub dead: i64,
    pub stale_running: i64,
    pub avg_duration_ms: Option<i64>,
    pub oldest_pending_seconds: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViolationDecision {
    pub id: String,
    pub violation_id: String,
    pub decision_type: String,
    pub decided_by: String,
    pub decided_at: i64,
    pub notes: Option<String>,
    pub evidence: serde_json::Value,
    pub created_at: i64,
}
