use crate::models::*;
use sha2::Digest;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
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

    pub async fn upsert_org_by_login(
        &self,
        login: &str,
        name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<String, DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO orgs (login, name, avatar_url)
            VALUES ($1, $2, $3)
            ON CONFLICT (login) DO UPDATE SET
                name = COALESCE($2, orgs.name),
                avatar_url = COALESCE($3, orgs.avatar_url),
                updated_at = NOW()
            RETURNING id::text
            "#,
        )
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

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name).await?.map(|r| r.id)
        } else {
            None
        };

        if filter.org_name.is_some() && org_id.is_none() {
            return Ok(vec![]);
        }
        if filter.repo_full_name.is_some() && repo_id.is_none() {
            return Ok(vec![]);
        }

        let mut query = String::from(
            "SELECT id::text, org_id::text, repo_id::text, delivery_id, event_type, actor_login, actor_id, ref_name, ref_type, before_sha, after_sha, commit_shas::text, commits_count, payload::text, created_at FROM github_events WHERE 1=1"
        );
        let mut param_count = 1;

        let mut conditions = Vec::new();

        if org_id.is_some() {
            conditions.push(format!("org_id = ${}", param_count));
            param_count += 1;
        }
        if repo_id.is_some() {
            conditions.push(format!("repo_id = ${}", param_count));
            param_count += 1;
        }
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
        if filter.branch.is_some() {
            conditions.push(format!("ref_name = ${}", param_count));
            param_count += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_count, param_count + 1));

        let mut sql_query = sqlx::query(&query);

        if let Some(ref org_id) = org_id {
            sql_query = sql_query.bind(org_id);
        }
        if let Some(ref repo_id) = repo_id {
            sql_query = sql_query.bind(repo_id);
        }
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
        if let Some(ref branch) = filter.branch {
            sql_query = sql_query.bind(branch);
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
        let mut in_batch_seen = HashSet::new();
        let mut deduped_events: Vec<&ClientEvent> = Vec::with_capacity(events.len());
        let mut duplicates: Vec<String> = Vec::new();

        for event in events {
            if !in_batch_seen.insert(event.event_uuid.clone()) {
                duplicates.push(event.event_uuid.clone());
                continue;
            }
            deduped_events.push(event);
        }

        match self.insert_client_events_batch_tx(&deduped_events).await {
            Ok(mut response) => {
                response.duplicates.extend(duplicates);
                Ok(response)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    batch_size = deduped_events.len(),
                    "Transactional client event batch insert failed, falling back to per-row inserts"
                );

                let mut accepted = Vec::new();
                let mut errors = Vec::new();
                for event in deduped_events {
                    match self.insert_client_event(event).await {
                        Ok(()) => accepted.push(event.event_uuid.clone()),
                        Err(DbError::Duplicate(_)) => duplicates.push(event.event_uuid.clone()),
                        Err(err) => errors.push(EventError {
                            event_uuid: event.event_uuid.clone(),
                            error: err.to_string(),
                        }),
                    }
                }

                Ok(ClientEventResponse {
                    accepted,
                    duplicates,
                    errors,
                })
            }
        }
    }

    async fn insert_client_events_batch_tx(
        &self,
        events: &[&ClientEvent],
    ) -> Result<ClientEventResponse, DbError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let mut accepted = Vec::new();
        let mut duplicates = Vec::new();
        let mut errors = Vec::new();

        for event in events {
            let files_json = match serde_json::to_string(&event.files) {
                Ok(json) => json,
                Err(e) => {
                    errors.push(EventError {
                        event_uuid: event.event_uuid.clone(),
                        error: DbError::SerializationError(e.to_string()).to_string(),
                    });
                    continue;
                }
            };

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
            .execute(&mut *tx)
            .await;

            match result {
                Ok(res) if res.rows_affected() == 0 => duplicates.push(event.event_uuid.clone()),
                Ok(_) => accepted.push(event.event_uuid.clone()),
                Err(e) if e.to_string().contains("duplicate") => duplicates.push(event.event_uuid.clone()),
                Err(e) => {
                    // Abort fast so caller can retry with per-row path preserving legacy behavior.
                    let _ = tx.rollback().await;
                    return Err(DbError::DatabaseError(e.to_string()));
                }
            }
        }

        tx.commit()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(ClientEventResponse {
            accepted,
            duplicates,
            errors,
        })
    }

    pub async fn get_client_events(&self, filter: &EventFilter) -> Result<Vec<ClientEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
        let offset = filter.offset as i64;

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name).await?.map(|r| r.id)
        } else {
            None
        };

        if filter.org_name.is_some() && org_id.is_none() {
            return Ok(vec![]);
        }
        if filter.repo_full_name.is_some() && repo_id.is_none() {
            return Ok(vec![]);
        }

        let mut query = String::from(
            "SELECT id::text, org_id::text, repo_id::text, event_uuid, event_type, user_login, user_name, branch, commit_sha, files::text, status, reason, metadata::text, client_version, created_at FROM client_events WHERE 1=1"
        );
        let mut param_count = 1;

        let mut conditions = Vec::new();

        if org_id.is_some() {
            conditions.push(format!("org_id = ${}", param_count));
            param_count += 1;
        }
        if repo_id.is_some() {
            conditions.push(format!("repo_id = ${}", param_count));
            param_count += 1;
        }
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
        if filter.branch.is_some() {
            conditions.push(format!("branch = ${}", param_count));
            param_count += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_count, param_count + 1));

        let mut sql_query = sqlx::query(&query);

        if let Some(ref org_id) = org_id {
            sql_query = sql_query.bind(org_id);
        }
        if let Some(ref repo_id) = repo_id {
            sql_query = sql_query.bind(repo_id);
        }
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
        if let Some(ref branch) = filter.branch {
            sql_query = sql_query.bind(branch);
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

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            // Fallback: handler may have set filter.org_id directly (UUID) to avoid a DB roundtrip.
            filter.org_id.clone()
        };

        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name).await?.map(|r| r.id)
        } else {
            None
        };

        // If caller requested a specific org/repo and it doesn't exist, return empty result.
        if filter.org_name.is_some() && org_id.is_none() {
            return Ok(vec![]);
        }
        if filter.repo_full_name.is_some() && repo_id.is_none() {
            return Ok(vec![]);
        }

        let start_date = filter
            .start_date
            .and_then(chrono::DateTime::from_timestamp_millis);
        let end_date = filter
            .end_date
            .and_then(chrono::DateTime::from_timestamp_millis);

        let result = sqlx::query(
            r#"
            SELECT id, source, event_type, created_at, user_login, repo_name, branch, status, details
            FROM (
                SELECT
                    g.id::TEXT AS id,
                    'github'::TEXT AS source,
                    g.event_type,
                    g.created_at,
                    g.actor_login AS user_login,
                    r.full_name AS repo_name,
                    g.ref_name AS branch,
                    NULL::TEXT AS status,
                    jsonb_build_object(
                        'commits_count', g.commits_count,
                        'after_sha', g.after_sha
                    ) AS details
                FROM github_events g
                LEFT JOIN repos r ON g.repo_id = r.id
                WHERE ($1::uuid IS NULL OR g.org_id = $1::uuid)
                  AND ($2::uuid IS NULL OR g.repo_id = $2::uuid)
                  AND ($3::text IS NULL OR $3 = 'github')
                  AND ($4::text IS NULL OR g.event_type = $4)
                  AND ($5::text IS NULL OR g.actor_login = $5)
                  AND ($6::text IS NULL OR g.ref_name = $6)
                  AND ($7::timestamptz IS NULL OR g.created_at >= $7)
                  AND ($8::timestamptz IS NULL OR g.created_at <= $8)
                  AND ($9::text IS NULL)

                UNION ALL

                SELECT
                    c.id::TEXT AS id,
                    'client'::TEXT AS source,
                    c.event_type,
                    c.created_at,
                    c.user_login,
                    r.full_name AS repo_name,
                    c.branch,
                    c.status,
                    jsonb_build_object(
                        'reason', c.reason,
                        'files', c.files
                    ) AS details
                FROM client_events c
                LEFT JOIN repos r ON c.repo_id = r.id
                WHERE ($1::uuid IS NULL OR c.org_id = $1::uuid)
                  AND ($2::uuid IS NULL OR c.repo_id = $2::uuid)
                  AND ($3::text IS NULL OR $3 = 'client')
                  AND ($4::text IS NULL OR c.event_type = $4)
                  AND ($5::text IS NULL OR c.user_login = $5)
                  AND ($6::text IS NULL OR c.branch = $6)
                  AND ($7::timestamptz IS NULL OR c.created_at >= $7)
                  AND ($8::timestamptz IS NULL OR c.created_at <= $8)
                  AND ($9::text IS NULL OR c.status = $9)
            ) combined
            ORDER BY created_at DESC
            LIMIT $10 OFFSET $11
            "#
        )
        .bind(&org_id)
        .bind(&repo_id)
        .bind(&filter.source)
        .bind(&filter.event_type)
        .bind(&filter.user_login)
        .bind(&filter.branch)
        .bind(start_date)
        .bind(end_date)
        .bind(&filter.status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let mut events: Vec<CombinedEvent> = result
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

        // Enrich client event details with metadata/commit_sha/user_name directly from client_events.
        // This avoids depending on DB function migrations for UI-visible fields like commit_message.
        let client_event_ids: Vec<String> = events
            .iter()
            .filter(|e| e.source == "client")
            .map(|e| e.id.clone())
            .collect();

        if !client_event_ids.is_empty() {
            let rows = sqlx::query(
                r#"
                SELECT id::text, commit_sha, user_name, COALESCE(metadata, '{}'::jsonb) as metadata
                FROM client_events
                WHERE id::text = ANY($1::text[])
                "#,
            )
            .bind(&client_event_ids)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

            let mut enrichment: HashMap<String, (Option<String>, Option<String>, serde_json::Value)> =
                HashMap::new();
            for row in rows {
                enrichment.insert(
                    row.get("id"),
                    (row.get("commit_sha"), row.get("user_name"), row.get("metadata")),
                );
            }

            for event in events.iter_mut().filter(|e| e.source == "client") {
                let Some((commit_sha, user_name, metadata)) = enrichment.get(&event.id) else {
                    continue;
                };

                let existing_details =
                    std::mem::replace(&mut event.details, serde_json::Value::Object(serde_json::Map::new()));

                let mut details_obj = match existing_details {
                    serde_json::Value::Object(map) => map,
                    serde_json::Value::Null => serde_json::Map::new(),
                    other => {
                        let mut map = serde_json::Map::new();
                        map.insert("legacy_details".to_string(), other);
                        map
                    }
                };

                if let Some(sha) = commit_sha {
                    details_obj
                        .entry("commit_sha".to_string())
                        .or_insert_with(|| serde_json::Value::String(sha.clone()));
                }

                if let Some(name) = user_name {
                    details_obj
                        .entry("user_name".to_string())
                        .or_insert_with(|| serde_json::Value::String(name.clone()));
                }

                match metadata {
                    serde_json::Value::Object(meta_obj) => {
                        for (k, v) in meta_obj {
                            details_obj.entry(k.clone()).or_insert_with(|| v.clone());
                        }
                    }
                    serde_json::Value::Null => {}
                    other => {
                        details_obj
                            .entry("metadata".to_string())
                            .or_insert_with(|| other.clone());
                    }
                }

                event.details = serde_json::Value::Object(details_obj);
            }
        }

        Ok(events)
    }

    /// Same as get_combined_events but without the 100-record default cap.
    /// Used for compliance exports — returns up to 50,000 records.
    pub async fn get_events_for_export(&self, filter: &EventFilter) -> Result<Vec<CombinedEvent>, DbError> {
        let limit = if filter.limit == 0 { 50_000_i32 } else { filter.limit.min(50_000) as i32 };
        let export_filter = EventFilter {
            limit: limit as usize,
            offset: 0,
            ..filter.clone()
        };
        self.get_combined_events(&export_filter).await
    }

    pub async fn list_export_logs(&self, org_id: Option<&str>) -> Result<Vec<ExportLog>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                exported_by,
                export_type,
                EXTRACT(EPOCH FROM date_range_start)::bigint * 1000 AS date_range_start_ms,
                EXTRACT(EPOCH FROM date_range_end)::bigint * 1000 AS date_range_end_ms,
                COALESCE(filters, 'null'::jsonb) AS filters,
                record_count,
                content_hash,
                file_path,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM export_logs
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let logs = rows
            .iter()
            .map(|row| ExportLog {
                id: row.get("id"),
                org_id: row.get("org_id"),
                exported_by: row.get("exported_by"),
                export_type: row.get("export_type"),
                date_range_start: row.get("date_range_start_ms"),
                date_range_end: row.get("date_range_end_ms"),
                filters: row.get("filters"),
                record_count: row.get("record_count"),
                content_hash: row.get("content_hash"),
                file_path: row.get("file_path"),
                created_at: row.get("created_at_ms"),
            })
            .collect();

        Ok(logs)
    }

    // ========================================================================
    // PIPELINE EVENTS (V1.2-A Jenkins Integration)
    // ========================================================================

    pub async fn insert_pipeline_event(&self, event: &PipelineEvent) -> Result<String, DbError> {
        let stages_json = serde_json::to_value(&event.stages)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;
        let artifacts_json = serde_json::to_value(&event.artifacts)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        let ingested_at = chrono::DateTime::from_timestamp_millis(event.ingested_at)
            .ok_or_else(|| DbError::SerializationError("Invalid ingested_at timestamp".to_string()))?;

        let result = sqlx::query(
            r#"
            INSERT INTO pipeline_events (
                id, org_id, pipeline_id, job_name, status, commit_sha, branch, repo_full_name,
                duration_ms, triggered_by, stages, artifacts, payload, ingested_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8,
                $9, $10, $11::jsonb, $12::jsonb, $13::jsonb, $14
            )
            ON CONFLICT (pipeline_id, job_name, (COALESCE(commit_sha, '')), ingested_at) DO NOTHING
            RETURNING id::text
            "#,
        )
        .bind(&event.id)
        .bind(&event.org_id)
        .bind(&event.pipeline_id)
        .bind(&event.job_name)
        .bind(event.status.as_str())
        .bind(&event.commit_sha)
        .bind(&event.branch)
        .bind(&event.repo_full_name)
        .bind(&event.duration_ms)
        .bind(&event.triggered_by)
        .bind(&stages_json)
        .bind(&artifacts_json)
        .bind(&event.payload)
        .bind(ingested_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        match result {
            Some(row) => Ok(row.get("id")),
            None => Err(DbError::Duplicate(format!(
                "pipeline_id={}, job_name={}, commit_sha={:?}, ingested_at={}",
                event.pipeline_id, event.job_name, event.commit_sha, event.ingested_at
            ))),
        }
    }

    pub async fn get_jenkins_integration_status(&self) -> Result<JenkinsIntegrationStatusResponse, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                MAX(ingested_at) AS last_ingest_at,
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '24 hours')::bigint AS recent_events_24h
            FROM pipeline_events
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let last_ingest_at = row
            .get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_ingest_at")
            .map(|dt| dt.timestamp_millis());
        let recent_events_24h: i64 = row.get("recent_events_24h");

        Ok(JenkinsIntegrationStatusResponse {
            ok: true,
            last_ingest_at,
            recent_events_24h,
        })
    }

    pub async fn upsert_project_ticket(&self, ticket: &ProjectTicket) -> Result<(), DbError> {
        let ingested_at = chrono::DateTime::from_timestamp_millis(ticket.ingested_at)
            .ok_or_else(|| DbError::SerializationError("Invalid ingested_at timestamp".to_string()))?;
        let created_at = ticket
            .created_at
            .and_then(chrono::DateTime::from_timestamp_millis);
        let updated_at = ticket
            .updated_at
            .and_then(chrono::DateTime::from_timestamp_millis);

        sqlx::query(
            r#"
            INSERT INTO project_tickets (
                id, org_id, ticket_id, ticket_url, title, status, assignee, reporter, priority, ticket_type,
                related_commits, related_prs, related_branches, created_at, updated_at, ingested_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10,
                $11::text[], $12::text[], $13::text[], $14, $15, $16
            )
            ON CONFLICT (org_id, ticket_id) DO UPDATE SET
                ticket_url = EXCLUDED.ticket_url,
                title = EXCLUDED.title,
                status = EXCLUDED.status,
                assignee = EXCLUDED.assignee,
                reporter = EXCLUDED.reporter,
                priority = EXCLUDED.priority,
                ticket_type = EXCLUDED.ticket_type,
                related_commits = EXCLUDED.related_commits,
                related_prs = EXCLUDED.related_prs,
                related_branches = EXCLUDED.related_branches,
                created_at = COALESCE(project_tickets.created_at, EXCLUDED.created_at),
                updated_at = EXCLUDED.updated_at,
                ingested_at = EXCLUDED.ingested_at
            "#,
        )
        .bind(&ticket.id)
        .bind(&ticket.org_id)
        .bind(&ticket.ticket_id)
        .bind(&ticket.ticket_url)
        .bind(&ticket.title)
        .bind(&ticket.status)
        .bind(&ticket.assignee)
        .bind(&ticket.reporter)
        .bind(&ticket.priority)
        .bind(&ticket.ticket_type)
        .bind(&ticket.related_commits)
        .bind(&ticket.related_prs)
        .bind(&ticket.related_branches)
        .bind(created_at)
        .bind(updated_at)
        .bind(ingested_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_jira_integration_status(&self) -> Result<JiraIntegrationStatusResponse, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                MAX(ingested_at) AS last_ingest_at,
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '24 hours')::bigint AS recent_tickets_24h
            FROM project_tickets
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(JiraIntegrationStatusResponse {
            ok: true,
            last_ingest_at: row
                .get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_ingest_at")
                .map(|dt| dt.timestamp_millis()),
            recent_tickets_24h: row.get("recent_tickets_24h"),
        })
    }

    pub async fn get_project_ticket_by_ticket_id(
        &self,
        ticket_id: &str,
    ) -> Result<Option<ProjectTicket>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text AS org_id,
                ticket_id,
                ticket_url,
                title,
                status,
                assignee,
                reporter,
                priority,
                ticket_type,
                related_commits,
                related_prs,
                related_branches,
                created_at,
                updated_at,
                ingested_at
            FROM project_tickets
            WHERE ticket_id = $1
            ORDER BY ingested_at DESC
            LIMIT 1
            "#,
        )
        .bind(ticket_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|row| {
            let created_at = row
                .get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")
                .map(|dt| dt.timestamp_millis());
            let updated_at = row
                .get::<Option<chrono::DateTime<chrono::Utc>>, _>("updated_at")
                .map(|dt| dt.timestamp_millis());
            let ingested_at = row
                .get::<chrono::DateTime<chrono::Utc>, _>("ingested_at")
                .timestamp_millis();

            ProjectTicket {
                id: row.get("id"),
                org_id: row.get("org_id"),
                ticket_id: row.get("ticket_id"),
                ticket_url: row.get("ticket_url"),
                title: row.get("title"),
                status: row.get("status"),
                assignee: row.get("assignee"),
                reporter: row.get("reporter"),
                priority: row.get("priority"),
                ticket_type: row.get("ticket_type"),
                related_commits: row.get("related_commits"),
                related_prs: row.get("related_prs"),
                related_branches: row.get("related_branches"),
                created_at,
                updated_at,
                ingested_at,
            }
        }))
    }

    pub async fn insert_commit_ticket_correlation(
        &self,
        correlation: &CommitTicketCorrelation,
    ) -> Result<bool, DbError> {
        let created_at = chrono::DateTime::from_timestamp_millis(correlation.created_at)
            .ok_or_else(|| DbError::SerializationError("Invalid created_at timestamp".to_string()))?;

        let result = sqlx::query(
            r#"
            INSERT INTO commit_ticket_correlations (
                id, org_id, commit_sha, ticket_id, correlation_source, confidence, created_at
            )
            VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)
            ON CONFLICT (commit_sha, ticket_id) DO NOTHING
            RETURNING id::text
            "#,
        )
        .bind(&correlation.id)
        .bind(&correlation.org_id)
        .bind(&correlation.commit_sha)
        .bind(&correlation.ticket_id)
        .bind(&correlation.correlation_source)
        .bind(correlation.confidence)
        .bind(created_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.is_some())
    }

    pub async fn append_project_ticket_relations(
        &self,
        ticket_id: &str,
        commit_sha: Option<&str>,
        branch: Option<&str>,
    ) -> Result<bool, DbError> {
        let commit_sha = commit_sha.map(str::trim).filter(|s| !s.is_empty());
        let branch = branch.map(str::trim).filter(|s| !s.is_empty());

        let result = sqlx::query(
            r#"
            UPDATE project_tickets
            SET
              related_commits = CASE
                WHEN $2::text IS NULL THEN related_commits
                ELSE (
                  SELECT COALESCE(array_agg(DISTINCT x), '{}'::text[])
                  FROM unnest(COALESCE(related_commits, '{}'::text[]) || ARRAY[$2::text]) AS x
                  WHERE x IS NOT NULL AND x <> ''
                )
              END,
              related_branches = CASE
                WHEN $3::text IS NULL THEN related_branches
                ELSE (
                  SELECT COALESCE(array_agg(DISTINCT x), '{}'::text[])
                  FROM unnest(COALESCE(related_branches, '{}'::text[]) || ARRAY[$3::text]) AS x
                  WHERE x IS NOT NULL AND x <> ''
                )
              END,
              updated_at = NOW()
            WHERE ticket_id = $1
            "#,
        )
        .bind(ticket_id)
        .bind(commit_sha)
        .bind(branch)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_recent_commit_events_for_ticket_correlation(
        &self,
        org_name: Option<&str>,
        repo_full_name: Option<&str>,
        hours: i64,
        limit: i64,
    ) -> Result<Vec<(String, Option<String>, Option<String>, serde_json::Value, Option<String>)>, DbError> {
        let org_id = if let Some(name) = org_name {
            self.get_org_by_login(name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(name) = repo_full_name {
            self.get_repo_by_full_name(name).await?.map(|r| r.id)
        } else {
            None
        };

        if org_name.is_some() && org_id.is_none() {
            return Ok(vec![]);
        }
        if repo_full_name.is_some() && repo_id.is_none() {
            return Ok(vec![]);
        }

        let rows = sqlx::query(
            r#"
            SELECT
                c.commit_sha,
                c.branch,
                c.org_id::text AS org_id,
                c.metadata,
                r.full_name AS repo_name
            FROM client_events c
            LEFT JOIN repos r ON r.id = c.repo_id
            WHERE c.event_type = 'commit'
              AND c.commit_sha IS NOT NULL
              AND c.created_at >= NOW() - make_interval(hours => $1::int)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
              AND ($3::uuid IS NULL OR c.repo_id = $3::uuid)
            ORDER BY c.created_at DESC
            LIMIT $4
            "#,
        )
        .bind(hours)
        .bind(&org_id)
        .bind(&repo_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get("commit_sha"),
                    row.get("branch"),
                    row.get("org_id"),
                    row.get("metadata"),
                    row.get("repo_name"),
                )
            })
            .collect())
    }

    pub async fn get_ticket_coverage(
        &self,
        org_name: Option<&str>,
        repo_full_name: Option<&str>,
        branch: Option<&str>,
        hours: i64,
    ) -> Result<TicketCoverageResponse, DbError> {
        let org_id = if let Some(name) = org_name {
            self.get_org_by_login(name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(name) = repo_full_name {
            self.get_repo_by_full_name(name).await?.map(|r| r.id)
        } else {
            None
        };
        if org_name.is_some() && org_id.is_none() {
            return Ok(TicketCoverageResponse {
                org: org_name.unwrap_or_default().to_string(),
                period: format!("last_{}h", hours),
                ..Default::default()
            });
        }
        if repo_full_name.is_some() && repo_id.is_none() {
            return Ok(TicketCoverageResponse {
                org: org_name.unwrap_or_default().to_string(),
                period: format!("last_{}h", hours),
                ..Default::default()
            });
        }

        let total_commits_row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS total
            FROM client_events c
            WHERE c.event_type = 'commit'
              AND c.commit_sha IS NOT NULL
              AND c.created_at >= NOW() - make_interval(hours => $1::int)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
              AND ($3::uuid IS NULL OR c.repo_id = $3::uuid)
              AND ($4::text IS NULL OR c.branch = $4)
            "#,
        )
        .bind(hours)
        .bind(&org_id)
        .bind(&repo_id)
        .bind(branch)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let with_ticket_row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT c.commit_sha)::bigint AS covered
            FROM client_events c
            JOIN commit_ticket_correlations ct ON ct.commit_sha = c.commit_sha
            WHERE c.event_type = 'commit'
              AND c.commit_sha IS NOT NULL
              AND c.created_at >= NOW() - make_interval(hours => $1::int)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
              AND ($3::uuid IS NULL OR c.repo_id = $3::uuid)
              AND ($4::text IS NULL OR c.branch = $4)
            "#,
        )
        .bind(hours)
        .bind(&org_id)
        .bind(&repo_id)
        .bind(branch)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let missing_rows = sqlx::query(
            r#"
            SELECT c.commit_sha, c.user_login, c.branch, c.created_at
            FROM client_events c
            LEFT JOIN commit_ticket_correlations ct ON ct.commit_sha = c.commit_sha
            WHERE c.event_type = 'commit'
              AND c.commit_sha IS NOT NULL
              AND c.created_at >= NOW() - make_interval(hours => $1::int)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
              AND ($3::uuid IS NULL OR c.repo_id = $3::uuid)
              AND ($4::text IS NULL OR c.branch = $4)
              AND ct.id IS NULL
            ORDER BY c.created_at DESC
            LIMIT 20
            "#,
        )
        .bind(hours)
        .bind(&org_id)
        .bind(&repo_id)
        .bind(branch)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let orphan_ticket_rows = sqlx::query(
            r#"
            SELECT pt.ticket_id, pt.status, pt.updated_at
            FROM project_tickets pt
            LEFT JOIN commit_ticket_correlations ct
              ON ct.ticket_id = pt.ticket_id
             AND (pt.org_id IS NULL OR ct.org_id = pt.org_id)
            WHERE pt.ingested_at >= NOW() - make_interval(hours => $1::int)
              AND ($2::uuid IS NULL OR pt.org_id = $2::uuid)
              AND ct.id IS NULL
            ORDER BY pt.ingested_at DESC
            LIMIT 20
            "#,
        )
        .bind(hours)
        .bind(&org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total_commits: i64 = total_commits_row.get("total");
        let commits_with_ticket: i64 = with_ticket_row.get("covered");
        let coverage_percentage = if total_commits > 0 {
            (commits_with_ticket as f64 / total_commits as f64) * 100.0
        } else {
            0.0
        };

        Ok(TicketCoverageResponse {
            org: org_name.unwrap_or("all").to_string(),
            period: format!("last_{}h", hours),
            total_commits,
            commits_with_ticket,
            coverage_percentage,
            commits_without_ticket: missing_rows
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "commit_sha": row.get::<String, _>("commit_sha"),
                        "user_login": row.get::<Option<String>, _>("user_login"),
                        "branch": row.get::<Option<String>, _>("branch"),
                        "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").timestamp_millis(),
                    })
                })
                .collect(),
            tickets_without_commits: orphan_ticket_rows
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "ticket_id": row.get::<String, _>("ticket_id"),
                        "status": row.get::<Option<String>, _>("status"),
                        "updated_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("updated_at")
                            .map(|dt| dt.timestamp_millis()),
                    })
                })
                .collect(),
        })
    }

    pub async fn get_commit_pipeline_correlations(
        &self,
        filter: &JenkinsCorrelationFilter,
    ) -> Result<Vec<CommitPipelineCorrelation>, DbError> {
        let limit = if filter.limit == 0 { 20 } else { filter.limit } as i64;
        let offset = filter.offset as i64;

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name).await?.map(|r| r.id)
        } else {
            None
        };

        if filter.org_name.is_some() && org_id.is_none() {
            return Ok(vec![]);
        }
        if filter.repo_full_name.is_some() && repo_id.is_none() {
            return Ok(vec![]);
        }

        let rows = sqlx::query(
            r#"
            SELECT
                c.id::text AS commit_event_id,
                c.commit_sha,
                c.created_at AS commit_created_at,
                c.user_login,
                c.branch,
                r.full_name AS repo_name,
                c.metadata AS metadata,
                p.id::text AS pipeline_event_id,
                p.pipeline_id,
                p.job_name,
                p.status AS pipeline_status,
                p.duration_ms AS pipeline_duration_ms,
                p.triggered_by,
                p.ingested_at AS pipeline_ingested_at
            FROM client_events c
            LEFT JOIN repos r ON r.id = c.repo_id
            LEFT JOIN LATERAL (
                SELECT pe.*
                FROM pipeline_events pe
                WHERE c.commit_sha IS NOT NULL
                  AND pe.commit_sha IS NOT NULL
                  AND (
                    pe.commit_sha = c.commit_sha
                    OR pe.commit_sha LIKE c.commit_sha || '%'
                    OR c.commit_sha LIKE pe.commit_sha || '%'
                  )
                ORDER BY pe.ingested_at DESC
                LIMIT 1
            ) p ON TRUE
            WHERE c.event_type = 'commit'
              AND c.commit_sha IS NOT NULL
              AND ($1::uuid IS NULL OR c.org_id = $1::uuid)
              AND ($2::uuid IS NULL OR c.repo_id = $2::uuid)
              AND ($3::text IS NULL OR c.branch = $3)
              AND ($4::text IS NULL OR c.user_login = $4)
            ORDER BY c.created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(&org_id)
        .bind(&repo_id)
        .bind(&filter.branch)
        .bind(&filter.user_login)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let correlations = rows
            .into_iter()
            .map(|row| {
                let commit_created_at: chrono::DateTime<chrono::Utc> = row.get("commit_created_at");
                let metadata: serde_json::Value = row.get("metadata");
                let commit_message = metadata
                    .as_object()
                    .and_then(|m| m.get("commit_message"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let pipeline = row
                    .get::<Option<String>, _>("pipeline_event_id")
                    .map(|pipeline_event_id| {
                        let ingested_at = row
                            .get::<Option<chrono::DateTime<chrono::Utc>>, _>("pipeline_ingested_at")
                            .map(|dt| dt.timestamp_millis())
                            .unwrap_or_default();

                        CommitPipelineRun {
                            pipeline_event_id,
                            pipeline_id: row.get("pipeline_id"),
                            job_name: row.get("job_name"),
                            status: row.get("pipeline_status"),
                            duration_ms: row.get("pipeline_duration_ms"),
                            triggered_by: row.get("triggered_by"),
                            ingested_at,
                        }
                    });

                CommitPipelineCorrelation {
                    commit_event_id: row.get("commit_event_id"),
                    commit_sha: row.get("commit_sha"),
                    commit_message,
                    commit_created_at: commit_created_at.timestamp_millis(),
                    user_login: row.get("user_login"),
                    branch: row.get("branch"),
                    repo_name: row.get("repo_name"),
                    pipeline,
                }
            })
            .collect();

        Ok(correlations)
    }

    pub async fn get_pipeline_health_stats(&self, org_id: Option<&str>) -> Result<PipelineHealthStats, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND ($1::uuid IS NULL OR org_id = $1::uuid))::bigint AS total_7d,
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND status = 'success' AND ($1::uuid IS NULL OR org_id = $1::uuid))::bigint AS success_7d,
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND status = 'failure' AND ($1::uuid IS NULL OR org_id = $1::uuid))::bigint AS failure_7d,
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND status = 'aborted' AND ($1::uuid IS NULL OR org_id = $1::uuid))::bigint AS aborted_7d,
                COUNT(*) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND status = 'unstable' AND ($1::uuid IS NULL OR org_id = $1::uuid))::bigint AS unstable_7d,
                COALESCE(AVG(duration_ms) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND duration_ms IS NOT NULL AND ($1::uuid IS NULL OR org_id = $1::uuid)), 0)::bigint AS avg_duration_ms_7d,
                COUNT(DISTINCT repo_full_name) FILTER (WHERE ingested_at >= NOW() - INTERVAL '7 days' AND status IN ('failure','unstable') AND repo_full_name IS NOT NULL AND ($1::uuid IS NULL OR org_id = $1::uuid))::bigint AS repos_with_failures_7d
            FROM pipeline_events
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await;

        match row {
            Ok(row) => Ok(PipelineHealthStats {
                total_7d: row.get("total_7d"),
                success_7d: row.get("success_7d"),
                failure_7d: row.get("failure_7d"),
                aborted_7d: row.get("aborted_7d"),
                unstable_7d: row.get("unstable_7d"),
                avg_duration_ms_7d: row.get("avg_duration_ms_7d"),
                repos_with_failures_7d: row.get("repos_with_failures_7d"),
            }),
            Err(e) => {
                // Keep compatibility if migration v5 was not applied yet.
                if e.to_string().contains("pipeline_events") {
                    Ok(PipelineHealthStats::default())
                } else {
                    Err(DbError::DatabaseError(e.to_string()))
                }
            }
        }
    }

    // ========================================================================
    // STATS
    // ========================================================================

    pub async fn get_stats(&self, org_id: Option<&str>) -> Result<AuditStats, DbError> {
        let result = sqlx::query("SELECT get_audit_stats($1::uuid) as stats")
            .bind(org_id)
            .fetch_one(&self.pool)
            .await;
        
        match result {
            Ok(row) => {
                let stats_json: Option<sqlx::types::Json<AuditStats>> = row.get("stats");
                Ok(stats_json.map(|j| j.0).unwrap_or_default())
            }
            Err(_) => {
                // Function might not exist or return null, return default stats
                Ok(AuditStats::default())
            }
        }
    }

    pub async fn get_desktop_pushes_today(&self, org_id: Option<&str>) -> Result<i64, DbError> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM client_events
            WHERE event_type = 'successful_push'
              AND ($1::uuid IS NULL OR org_id = $1::uuid)
              AND created_at >= DATE_TRUNC('day', NOW())
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(count)
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
            SELECT client_id, role, org_id::text, last_used
            FROM api_keys
            WHERE key_hash = $1 AND is_active = TRUE
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

                // Reduce write amplification on high-traffic endpoints.
                // `last_used` is observability metadata, so update only every ~5 minutes.
                let last_used: Option<chrono::DateTime<chrono::Utc>> = row.get("last_used");
                let should_update_last_used = last_used
                    .map(|ts| chrono::Utc::now().signed_duration_since(ts).num_minutes() >= 5)
                    .unwrap_or(true);

                if should_update_last_used {
                    if let Err(e) = sqlx::query(
                        r#"
                        UPDATE api_keys
                        SET last_used = NOW()
                        WHERE key_hash = $1
                          AND is_active = TRUE
                          AND (last_used IS NULL OR last_used < NOW() - INTERVAL '5 minutes')
                        "#,
                    )
                    .bind(key_hash)
                    .execute(&self.pool)
                    .await
                    {
                        tracing::warn!(
                            error = %e,
                            client_id = %client_id,
                            "Failed to update api_keys.last_used (non-fatal)"
                        );
                    }
                }

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

    pub async fn list_api_keys(&self, org_id: Option<&str>) -> Result<Vec<ApiKeyInfo>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                client_id,
                role,
                org_id::text,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM last_used)::bigint * 1000 AS last_used_ms,
                is_active
            FROM api_keys
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
            ORDER BY created_at DESC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let keys = rows
            .iter()
            .map(|row| ApiKeyInfo {
                id: row.get("id"),
                client_id: row.get("client_id"),
                role: row.get("role"),
                org_id: row.get("org_id"),
                created_at: row.get::<i64, _>("created_at_ms"),
                last_used: row.get("last_used_ms"),
                is_active: row.get("is_active"),
            })
            .collect();

        Ok(keys)
    }

    pub async fn revoke_api_key(&self, id: &str, org_id: Option<&str>) -> Result<bool, DbError> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys
            SET is_active = FALSE
            WHERE
                id = $1::uuid
                AND is_active = TRUE
                AND ($2::uuid IS NULL OR org_id = $2::uuid)
            "#,
        )
        .bind(id)
        .bind(org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    // ========================================================================
    // PULL REQUEST MERGES
    // ========================================================================

    pub async fn insert_pr_merge(&self, record: &PrMergeRecord) -> Result<(), DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO pull_request_merges (
                id, org_id, repo_id, delivery_id, pr_number, pr_title,
                author_login, merged_by_login, head_sha, base_branch, payload
            )
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10, $11::jsonb)
            ON CONFLICT (delivery_id) DO NOTHING
            "#,
        )
        .bind(&record.id)
        .bind(&record.org_id)
        .bind(&record.repo_id)
        .bind(&record.delivery_id)
        .bind(record.pr_number)
        .bind(&record.pr_title)
        .bind(&record.author_login)
        .bind(&record.merged_by_login)
        .bind(&record.head_sha)
        .bind(&record.base_branch)
        .bind(&record.payload)
        .execute(&self.pool)
        .await;

        match result {
            Ok(res) if res.rows_affected() == 0 => {
                Err(DbError::Duplicate(format!("delivery_id: {}", record.delivery_id)))
            }
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => {
                Err(DbError::Duplicate(format!("delivery_id: {}", record.delivery_id)))
            }
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn list_pr_merge_evidence(
        &self,
        scope_org_id: Option<&str>,
        org_name: Option<&str>,
        repo_full_name: Option<&str>,
        merged_by: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<PrMergeEvidenceEntry>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                prm.id::text AS id,
                prm.org_id::text AS org_id,
                o.login AS org_name,
                prm.repo_id::text AS repo_id,
                r.full_name AS repo_full_name,
                prm.delivery_id,
                prm.pr_number,
                prm.pr_title,
                prm.author_login,
                prm.merged_by_login,
                prm.head_sha,
                prm.base_branch,
                prm.payload,
                EXTRACT(EPOCH FROM prm.created_at)::bigint * 1000 AS created_at_ms
            FROM pull_request_merges prm
            LEFT JOIN orgs o ON o.id = prm.org_id
            LEFT JOIN repos r ON r.id = prm.repo_id
            WHERE ($1::uuid IS NULL OR prm.org_id = $1::uuid)
              AND ($2::text IS NULL OR o.login = $2)
              AND ($3::text IS NULL OR r.full_name = $3)
              AND ($4::text IS NULL OR prm.merged_by_login = $4)
            ORDER BY prm.created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(scope_org_id)
        .bind(org_name)
        .bind(repo_full_name)
        .bind(merged_by)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM pull_request_merges prm
            LEFT JOIN orgs o ON o.id = prm.org_id
            LEFT JOIN repos r ON r.id = prm.repo_id
            WHERE ($1::uuid IS NULL OR prm.org_id = $1::uuid)
              AND ($2::text IS NULL OR o.login = $2)
              AND ($3::text IS NULL OR r.full_name = $3)
              AND ($4::text IS NULL OR prm.merged_by_login = $4)
            "#,
        )
        .bind(scope_org_id)
        .bind(org_name)
        .bind(repo_full_name)
        .bind(merged_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");

        let entries = rows
            .into_iter()
            .map(|row| {
                let payload: serde_json::Value = row.get("payload");
                let approvers = payload
                    .pointer("/gitgov/approvers")
                    .cloned()
                    .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                    .unwrap_or_default();
                let approvals_count = payload
                    .pointer("/gitgov/approvals_count")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32)
                    .unwrap_or(approvers.len() as i32);

                PrMergeEvidenceEntry {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    org_name: row.get("org_name"),
                    repo_id: row.get("repo_id"),
                    repo_full_name: row.get("repo_full_name"),
                    delivery_id: row.get("delivery_id"),
                    pr_number: row.get("pr_number"),
                    pr_title: row.get("pr_title"),
                    author_login: row.get("author_login"),
                    merged_by_login: row.get("merged_by_login"),
                    approvers,
                    approvals_count,
                    head_sha: row.get("head_sha"),
                    base_branch: row.get("base_branch"),
                    created_at: row.get::<i64, _>("created_at_ms"),
                }
            })
            .collect();

        Ok((entries, total))
    }

    // ========================================================================
    // ADMIN AUDIT LOG
    // ========================================================================

    pub async fn insert_admin_audit_log(&self, entry: &AdminAuditLogEntry) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO admin_audit_log (id, actor_client_id, action, target_type, target_id, metadata)
            VALUES ($1::uuid, $2, $3, $4, $5, $6::jsonb)
            "#,
        )
        .bind(&entry.id)
        .bind(&entry.actor_client_id)
        .bind(&entry.action)
        .bind(&entry.target_type)
        .bind(&entry.target_id)
        .bind(&entry.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn list_admin_audit_logs(
        &self,
        actor: Option<&str>,
        action_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<AdminAuditLogEntry>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                actor_client_id,
                action,
                target_type,
                target_id,
                COALESCE(metadata, '{}') AS metadata,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM admin_audit_log
            WHERE ($1::text IS NULL OR actor_client_id = $1)
              AND ($2::text IS NULL OR action = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(actor)
        .bind(action_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM admin_audit_log
            WHERE ($1::text IS NULL OR actor_client_id = $1)
              AND ($2::text IS NULL OR action = $2)
            "#,
        )
        .bind(actor)
        .bind(action_filter)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");

        let entries = rows
            .iter()
            .map(|row| AdminAuditLogEntry {
                id: row.get("id"),
                actor_client_id: row.get("actor_client_id"),
                action: row.get("action"),
                target_type: row.get("target_type"),
                target_id: row.get("target_id"),
                metadata: row.get("metadata"),
                created_at: row.get::<i64, _>("created_at_ms"),
            })
            .collect();

        Ok((entries, total))
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
        actor_login: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<NoncomplianceSignal>, i64), DbError> {
        let mut conditions = Vec::new();
        let mut param_count = 1;

        if org_name.is_some() {
            conditions.push(format!("ns.org_id = (SELECT id FROM orgs WHERE login = ${})", param_count));
            param_count += 1;
        }

        if confidence.is_some() {
            conditions.push(format!("ns.confidence = ${}", param_count));
            param_count += 1;
        }
        if status.is_some() {
            conditions.push(format!(
                "COALESCE((SELECT sd.decision FROM signal_decisions sd WHERE sd.signal_id = ns.id ORDER BY sd.created_at DESC LIMIT 1), ns.status) = ${}",
                param_count
            ));
            param_count += 1;
        }
        if signal_type.is_some() {
            conditions.push(format!("ns.signal_type = ${}", param_count));
            param_count += 1;
        }
        if actor_login.is_some() {
            conditions.push(format!("ns.actor_login = ${}", param_count));
            param_count += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let count_query = format!("SELECT COUNT(*) as total FROM noncompliance_signals ns{}", where_clause);
        
        let mut count_sql = sqlx::query(&count_query);
        
        if let Some(org) = org_name {
            count_sql = count_sql.bind(org);
        }
        if let Some(c) = confidence {
            count_sql = count_sql.bind(c);
        }
        if let Some(s) = status {
            count_sql = count_sql.bind(s);
        }
        if let Some(st) = signal_type {
            count_sql = count_sql.bind(st);
        }
        if let Some(actor) = actor_login {
            count_sql = count_sql.bind(actor);
        }

        let count_row = count_sql
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let total: i64 = count_row.get("total");

        let data_query = format!(
            "SELECT ns.id::text, ns.org_id::text, ns.repo_id::text, ns.github_event_id::text, ns.client_event_id::text, \
             ns.signal_type, ns.confidence, ns.actor_login, ns.branch, ns.commit_sha, ns.evidence, ns.context, \
             COALESCE(sd.decision, ns.status) as status, \
             COALESCE(sd.decided_by, ns.investigated_by) as investigated_by, \
             COALESCE(sd.created_at, ns.investigated_at) as investigated_at, \
             COALESCE(sd.notes, ns.investigation_notes) as investigation_notes, \
             ns.created_at \
             FROM noncompliance_signals ns \
             LEFT JOIN LATERAL ( \
                SELECT decision, decided_by, notes, created_at \
                FROM signal_decisions \
                WHERE signal_id = ns.id \
                ORDER BY created_at DESC \
                LIMIT 1 \
             ) sd ON true{} ORDER BY ns.created_at DESC LIMIT ${} OFFSET ${}",
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
        if let Some(actor) = actor_login {
            data_sql = data_sql.bind(actor);
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
        decided_by: &str,
        notes: Option<&str>,
    ) -> Result<(), DbError> {
        // Preferred path (append-only): record a decision in signal_decisions.
        // This works with schemas that forbid UPDATE on noncompliance_signals.
        let decision_insert = sqlx::query(
            r#"
            INSERT INTO signal_decisions (id, signal_id, decision, decided_by, notes, created_at)
            VALUES ($1::uuid, $2::uuid, $3, $4, $5, NOW())
            "#,
        )
        .bind(&uuid::Uuid::new_v4().to_string())
        .bind(signal_id)
        .bind(status)
        .bind(decided_by)
        .bind(notes)
        .execute(&self.pool)
        .await;

        match decision_insert {
            Ok(_) => {
                // Legacy compatibility: if schema still allows mutable fields, mirror latest status.
                // Ignore failures here because append-only schemas intentionally reject UPDATE.
                if let Err(e) = sqlx::query(
                    r#"
                    UPDATE noncompliance_signals 
                    SET status = $2, 
                        investigated_by = $3,
                        investigation_notes = $4,
                        investigated_at = NOW()
                    WHERE id = $1::uuid
                    "#,
                )
                .bind(signal_id)
                .bind(status)
                .bind(decided_by)
                .bind(notes)
                .execute(&self.pool)
                .await
                {
                    tracing::debug!(
                        signal_id = %signal_id,
                        error = %e,
                        "Legacy noncompliance_signals update skipped after decision insert"
                    );
                }

                Ok(())
            }
            Err(insert_err) => {
                let insert_err_msg = insert_err.to_string();

                // Fallback for older schemas without signal_decisions table.
                if insert_err_msg.contains("signal_decisions") && insert_err_msg.contains("does not exist") {
                    sqlx::query(
                        r#"
                        UPDATE noncompliance_signals 
                        SET status = $2, 
                            investigated_by = $3,
                            investigation_notes = $4,
                            investigated_at = NOW()
                        WHERE id = $1::uuid
                        "#,
                    )
                    .bind(signal_id)
                    .bind(status)
                    .bind(decided_by)
                    .bind(notes)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| DbError::DatabaseError(e.to_string()))?;

                    return Ok(());
                }

                Err(DbError::DatabaseError(insert_err_msg))
            }
        }
    }

    pub async fn get_signal_by_id(&self, signal_id: &str) -> Result<Option<NoncomplianceSignal>, DbError> {
        let result = sqlx::query(
            r#"
            SELECT ns.id::text, ns.org_id::text, ns.repo_id::text, ns.github_event_id::text, ns.client_event_id::text,
                   ns.signal_type, ns.confidence, ns.actor_login, ns.branch, ns.commit_sha, ns.evidence, ns.context,
                   COALESCE(sd.decision, ns.status) as status,
                   COALESCE(sd.decided_by, ns.investigated_by) as investigated_by,
                   COALESCE(sd.created_at, ns.investigated_at) as investigated_at,
                   COALESCE(sd.notes, ns.investigation_notes) as investigation_notes,
                   ns.created_at
            FROM noncompliance_signals ns
            LEFT JOIN LATERAL (
                SELECT decision, decided_by, notes, created_at
                FROM signal_decisions
                WHERE signal_id = ns.id
                ORDER BY created_at DESC
                LIMIT 1
            ) sd ON true
            WHERE ns.id = $1::uuid
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

        let result = sqlx::query(
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

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("Job not found: {}", job_id)));
        }

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
                return Err(DbError::NotFound(format!("Job not found: {}", job_id)));
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
        let result = sqlx::query(
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

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound(format!("Dead job not found: {}", job_id)));
        }

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
        let function_call = sqlx::query(
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
        .bind(evidence.clone().unwrap_or(serde_json::Value::Null))
        .fetch_one(&self.pool)
        .await;

        let decision_id: String = match function_call {
            Ok(result) => result.get("decision_id"),
            Err(function_err) => {
                tracing::warn!(
                    violation_id = %violation_id,
                    decision_type = %decision_type,
                    error = %function_err,
                    "Falling back to direct violation_decisions upsert"
                );

                let row = sqlx::query(
                    r#"
                    INSERT INTO violation_decisions (
                        violation_id, decision_type, decided_by, notes, evidence
                    ) VALUES (
                        $1::uuid, $2, $3, $4, $5
                    )
                    ON CONFLICT (violation_id, decision_type) DO UPDATE SET
                        decided_by = EXCLUDED.decided_by,
                        decided_at = NOW(),
                        notes = EXCLUDED.notes,
                        evidence = EXCLUDED.evidence
                    RETURNING id::text as decision_id
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

                row.get("decision_id")
            }
        };
        
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

    pub async fn get_violation_scope(
        &self,
        violation_id: &str,
    ) -> Result<Option<(Option<String>, Option<String>)>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT org_id::text, user_login
            FROM violations
            WHERE id = $1::uuid
            "#,
        )
        .bind(violation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| (r.get("org_id"), r.get("user_login"))))
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
