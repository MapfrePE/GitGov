use crate::models::*;
use sha2::Digest;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, QueryBuilder, Row};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
    auth_cache: Arc<Mutex<HashMap<String, CachedApiKeyAuth>>>,
    auth_cache_ttl: Duration,
    auth_cache_stale_max: Duration,
    auth_cache_max_entries: usize,
    auth_db_failure_streak: Arc<AtomicU32>,
    auth_stale_fail_closed_after: u32,
}

#[derive(Clone)]
struct CachedApiKeyAuth {
    value: Option<ApiKeyAuthCacheValue>,
    cached_at: Instant,
}

type ApiKeyAuthCacheValue = (String, UserRole, Option<String>);
type StaleApiKeyAuthCacheValue = (ApiKeyAuthCacheValue, u64);

#[derive(Debug, Clone)]
pub struct ApiKeyAuthValidation {
    pub auth: Option<ApiKeyAuthCacheValue>,
    pub used_stale_cache: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DistributedRateLimitCheck {
    pub allowed: bool,
    pub retry_after_secs: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct OutboxLeaseDecision {
    pub granted: bool,
    pub wait_ms: u64,
}

const SIMULATE_AUTH_DB_FAILURE_ENV: &str = "GITGOV_SIMULATE_AUTH_DB_FAILURE";
const SIMULATE_AUTH_DB_FAILURE_FLAG_FILE_ENV: &str = "GITGOV_SIMULATE_AUTH_DB_FAILURE_FLAG_FILE";

fn parse_bool_like(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn auth_db_failure_simulation_enabled() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }

    let force_failure = std::env::var(SIMULATE_AUTH_DB_FAILURE_ENV)
        .ok()
        .as_deref()
        .map(parse_bool_like)
        .unwrap_or(false);
    if force_failure {
        return true;
    }

    let Some(flag_file) = std::env::var(SIMULATE_AUTH_DB_FAILURE_FLAG_FILE_ENV).ok() else {
        return false;
    };
    let trimmed = flag_file.trim();
    if trimmed.is_empty() {
        return false;
    }

    std::path::Path::new(trimmed).exists()
}

pub struct NoncomplianceSignalsQuery<'a> {
    pub org_id: Option<&'a str>,
    pub confidence: Option<&'a str>,
    pub status: Option<&'a str>,
    pub signal_type: Option<&'a str>,
    pub actor_login: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

pub struct UpsertOrgUserInput<'a> {
    pub org_id: &'a str,
    pub login: &'a str,
    pub display_name: Option<&'a str>,
    pub email: Option<&'a str>,
    pub role: &'a str,
    pub status: &'a str,
    pub actor: &'a str,
}

pub struct CreateOrgInvitationInput<'a> {
    pub org_id: &'a str,
    pub invite_email: Option<&'a str>,
    pub invite_login: Option<&'a str>,
    pub role: &'a str,
    pub token_hash: &'a str,
    pub invited_by: &'a str,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub struct CreatePolicyChangeRequestInput<'a> {
    pub request_id: &'a str,
    pub org_id: Option<&'a str>,
    pub repo_id: &'a str,
    pub repo_name: &'a str,
    pub requested_by: &'a str,
    pub requested_config: &'a GitGovConfig,
    pub requested_checksum: &'a str,
    pub reason: Option<&'a str>,
    pub created_at: i64,
}

pub struct ListPolicyChangeRequestsInput<'a> {
    pub org_id: Option<&'a str>,
    pub repo_name: Option<&'a str>,
    pub requested_by: Option<&'a str>,
    pub status: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
    pub include_config: bool,
}

#[derive(Debug, Clone)]
pub struct AcceptedOrgInvitation {
    pub invitation: OrgInvitation,
    pub org_user: OrgUser,
    pub api_key: String,
}

impl Database {
    /// Create a Database from an existing PgPool (used by integration tests).
    #[cfg(test)]
    pub fn from_pool(pool: PgPool) -> Self {
        Self {
            pool,
            auth_cache: Arc::new(Mutex::new(HashMap::new())),
            auth_cache_ttl: Duration::from_secs(20),
            auth_cache_stale_max: Duration::from_secs(120),
            auth_cache_max_entries: 4096,
            auth_db_failure_streak: Arc::new(AtomicU32::new(0)),
            auth_stale_fail_closed_after: 0,
        }
    }

    pub async fn new(database_url: &str) -> Result<Self, DbError> {
        let runtime_env = std::env::var("GITGOV_ENV")
            .unwrap_or_else(|_| "dev".to_string())
            .trim()
            .to_ascii_lowercase();
        let is_dev_env = matches!(
            runtime_env.as_str(),
            "dev" | "development" | "local" | "test"
        );
        let max_connections = std::env::var("GITGOV_DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(20)
            .max(1);
        let min_connections = std::env::var("GITGOV_DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(2)
            .min(max_connections);
        let acquire_timeout_secs = std::env::var("GITGOV_DB_ACQUIRE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(8)
            .max(1);
        let idle_timeout_secs = std::env::var("GITGOV_DB_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300)
            .max(10);
        let max_lifetime_secs = std::env::var("GITGOV_DB_MAX_LIFETIME_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1800)
            .max(60);
        let auth_cache_ttl_secs = std::env::var("GITGOV_AUTH_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(20)
            .clamp(1, 300);
        let auth_cache_max_entries = std::env::var("GITGOV_AUTH_CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4096)
            .max(64);
        let auth_cache_stale_max_secs = std::env::var("GITGOV_AUTH_CACHE_STALE_MAX_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(if is_dev_env { 120 } else { 30 })
            .clamp(auth_cache_ttl_secs, 900);
        let auth_stale_fail_closed_after =
            std::env::var("GITGOV_AUTH_STALE_FAIL_CLOSED_AFTER_DB_ERRORS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(if is_dev_env { 0 } else { 3 })
                .min(10_000);

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
            .idle_timeout(Some(Duration::from_secs(idle_timeout_secs)))
            .max_lifetime(Some(Duration::from_secs(max_lifetime_secs)))
            .connect(database_url)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(Self {
            pool,
            auth_cache: Arc::new(Mutex::new(HashMap::new())),
            auth_cache_ttl: Duration::from_secs(auth_cache_ttl_secs),
            auth_cache_stale_max: Duration::from_secs(auth_cache_stale_max_secs),
            auth_cache_max_entries,
            auth_db_failure_streak: Arc::new(AtomicU32::new(0)),
            auth_stale_fail_closed_after,
        })
    }

    fn get_cached_api_key_auth_with_max_age(
        &self,
        key_hash: &str,
        max_age: Duration,
    ) -> Option<Option<ApiKeyAuthCacheValue>> {
        let cache = self.auth_cache.lock().ok()?;
        let entry = cache.get(key_hash).cloned()?;
        if entry.cached_at.elapsed() <= max_age {
            return Some(entry.value);
        }
        None
    }

    fn get_cached_api_key_auth(&self, key_hash: &str) -> Option<Option<ApiKeyAuthCacheValue>> {
        self.get_cached_api_key_auth_with_max_age(key_hash, self.auth_cache_ttl)
    }

    fn get_stale_cached_api_key_auth(&self, key_hash: &str) -> Option<StaleApiKeyAuthCacheValue> {
        let mut cache = self.auth_cache.lock().ok()?;
        let entry = cache.get(key_hash).cloned()?;
        let age = entry.cached_at.elapsed();
        if age <= self.auth_cache_stale_max {
            return entry.value.map(|auth| (auth, age.as_secs()));
        }
        cache.remove(key_hash);
        None
    }

    fn put_cached_api_key_auth(&self, key_hash: &str, value: Option<ApiKeyAuthCacheValue>) {
        if let Ok(mut cache) = self.auth_cache.lock() {
            if cache.len() >= self.auth_cache_max_entries && !cache.contains_key(key_hash) {
                if let Some(stale_key) = cache.iter().find_map(|(k, v)| {
                    (v.cached_at.elapsed() > self.auth_cache_ttl).then(|| k.clone())
                }) {
                    cache.remove(&stale_key);
                } else if let Some(first_key) = cache.keys().next().cloned() {
                    cache.remove(&first_key);
                }
            }

            cache.insert(
                key_hash.to_string(),
                CachedApiKeyAuth {
                    value,
                    cached_at: Instant::now(),
                },
            );
        }
    }

    fn invalidate_auth_cache_key(&self, key_hash: &str) {
        if let Ok(mut cache) = self.auth_cache.lock() {
            cache.remove(key_hash);
        }
    }

    fn invalidate_auth_cache_all(&self) {
        if let Ok(mut cache) = self.auth_cache.lock() {
            cache.clear();
        }
    }

    fn note_auth_db_failure(&self) -> (u32, bool) {
        let streak = self
            .auth_db_failure_streak
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        let should_fail_closed =
            self.auth_stale_fail_closed_after > 0 && streak >= self.auth_stale_fail_closed_after;
        (streak, should_fail_closed)
    }

    fn reset_auth_db_failure_streak(&self) {
        self.auth_db_failure_streak.store(0, Ordering::Relaxed);
    }

    pub async fn ensure_rate_limit_storage(&self) -> Result<(), DbError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS rate_limit_counters (
                limiter_name TEXT NOT NULL,
                scope_key TEXT NOT NULL,
                window_start TIMESTAMPTZ NOT NULL,
                count INTEGER NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (limiter_name, scope_key, window_start)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_rate_limit_counters_updated_at
            ON rate_limit_counters (updated_at)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn ensure_outbox_lease_storage(&self) -> Result<(), DbError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS outbox_flush_leases (
                lease_key TEXT PRIMARY KEY,
                holder TEXT NOT NULL,
                lease_until TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_outbox_flush_leases_updated_at
            ON outbox_flush_leases (updated_at)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn try_acquire_outbox_flush_lease(
        &self,
        lease_key: &str,
        holder: &str,
        lease_ttl: Duration,
    ) -> Result<OutboxLeaseDecision, DbError> {
        let ttl_ms = lease_ttl.as_millis().clamp(1, i64::MAX as u128) as i64;
        let row = sqlx::query(
            r#"
            INSERT INTO outbox_flush_leases (lease_key, holder, lease_until, updated_at)
            VALUES (
                $1::text,
                $2::text,
                NOW() + ($3::bigint * INTERVAL '1 millisecond'),
                NOW()
            )
            ON CONFLICT (lease_key) DO UPDATE
            SET
                holder = CASE
                    WHEN outbox_flush_leases.lease_until <= NOW()
                        OR outbox_flush_leases.holder = EXCLUDED.holder
                    THEN EXCLUDED.holder
                    ELSE outbox_flush_leases.holder
                END,
                lease_until = CASE
                    WHEN outbox_flush_leases.lease_until <= NOW()
                        OR outbox_flush_leases.holder = EXCLUDED.holder
                    THEN EXCLUDED.lease_until
                    ELSE outbox_flush_leases.lease_until
                END,
                updated_at = NOW()
            RETURNING holder, lease_until, NOW() AS now_ts
            "#,
        )
        .bind(lease_key)
        .bind(holder)
        .bind(ttl_ms)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let granted_holder: String = row
            .try_get("holder")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let lease_until: chrono::DateTime<chrono::Utc> = row
            .try_get("lease_until")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let now_ts: chrono::DateTime<chrono::Utc> = row
            .try_get("now_ts")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let granted = granted_holder == holder;
        let wait_ms = if granted {
            0
        } else {
            lease_until
                .signed_duration_since(now_ts)
                .num_milliseconds()
                .max(1) as u64
        };

        Ok(OutboxLeaseDecision { granted, wait_ms })
    }

    pub async fn prune_rate_limit_counters(&self, retention: Duration) -> Result<u64, DbError> {
        if retention.is_zero() {
            return Ok(0);
        }
        let retention_secs = retention.as_secs().min(i64::MAX as u64) as i64;
        let result = sqlx::query(
            r#"
            DELETE FROM rate_limit_counters
            WHERE updated_at < NOW() - make_interval(secs => $1)
            "#,
        )
        .bind(retention_secs)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        Ok(result.rows_affected())
    }

    pub async fn publish_sse_notification(
        &self,
        channel: &str,
        payload: &str,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            SELECT pg_notify($1::text, $2::text)
            "#,
        )
        .bind(channel)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn check_distributed_rate_limit(
        &self,
        limiter_name: &str,
        scope_key: &str,
        limit: u32,
        window: Duration,
    ) -> Result<DistributedRateLimitCheck, DbError> {
        if limit == 0 {
            return Ok(DistributedRateLimitCheck {
                allowed: true,
                retry_after_secs: 0,
            });
        }

        let window_secs = window.as_secs().max(1).min(i64::MAX as u64) as i64;
        let limit_i64 = limit as i64;

        let row = sqlx::query(
            r#"
            WITH params AS (
                SELECT
                    $1::text AS limiter_name,
                    $2::text AS scope_key,
                    $3::bigint AS limit_count,
                    $4::bigint AS window_secs,
                    NOW() AS now_ts
            ),
            bucket AS (
                SELECT
                    limiter_name,
                    scope_key,
                    limit_count,
                    window_secs,
                    now_ts,
                    to_timestamp(floor(extract(epoch FROM now_ts) / window_secs) * window_secs) AS window_start
                FROM params
            ),
            upsert AS (
                INSERT INTO rate_limit_counters (limiter_name, scope_key, window_start, count, updated_at)
                SELECT limiter_name, scope_key, window_start, 1, now_ts
                FROM bucket
                ON CONFLICT (limiter_name, scope_key, window_start)
                DO UPDATE
                SET count = rate_limit_counters.count + 1,
                    updated_at = EXCLUDED.updated_at
                RETURNING count, window_start
            )
            SELECT
                upsert.count::bigint AS current_count,
                upsert.window_start,
                bucket.now_ts,
                bucket.window_secs,
                bucket.limit_count
            FROM upsert
            CROSS JOIN bucket
            "#,
        )
        .bind(limiter_name)
        .bind(scope_key)
        .bind(limit_i64)
        .bind(window_secs)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let current_count: i64 = row
            .try_get("current_count")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let window_start: chrono::DateTime<chrono::Utc> = row
            .try_get("window_start")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let now_ts: chrono::DateTime<chrono::Utc> = row
            .try_get("now_ts")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let window_secs_row: i64 = row
            .try_get("window_secs")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        let limit_count: i64 = row
            .try_get("limit_count")
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        if current_count <= limit_count {
            return Ok(DistributedRateLimitCheck {
                allowed: true,
                retry_after_secs: 0,
            });
        }

        let elapsed_secs = now_ts
            .signed_duration_since(window_start)
            .num_seconds()
            .max(0);
        let retry_after_secs = (window_secs_row - elapsed_secs).max(1) as u64;

        Ok(DistributedRateLimitCheck {
            allowed: false,
            retry_after_secs,
        })
    }

    // ========================================================================
    // ORGANIZATIONS
    // ========================================================================

    pub async fn upsert_org(
        &self,
        github_id: i64,
        login: &str,
        name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<String, DbError> {
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

    pub async fn upsert_repo_by_full_name(
        &self,
        org_id: Option<&str>,
        full_name: &str,
        name: &str,
        private: bool,
    ) -> Result<String, DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO repos (org_id, github_id, full_name, name, private)
            VALUES ($1::uuid, NULL, $2, $3, $4)
            ON CONFLICT (full_name) DO UPDATE SET
                org_id = COALESCE(repos.org_id, $1::uuid),
                name = $3,
                private = $4,
                updated_at = NOW()
            RETURNING id::text
            "#,
        )
        .bind(org_id)
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
        .bind(event.actor_id)
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
            Ok(res) if res.rows_affected() == 0 => Err(DbError::Duplicate(format!(
                "delivery_id: {}",
                event.delivery_id
            ))),
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => Err(DbError::Duplicate(format!(
                "delivery_id: {}",
                event.delivery_id
            ))),
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn get_github_events(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<GitHubEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
        let offset = filter.offset as i64;

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name)
                .await?
                .map(|r| r.id)
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
            conditions.push(format!(
                "created_at >= to_timestamp(${0}/1000.0)",
                param_count
            ));
            param_count += 1;
        }
        if filter.end_date.is_some() {
            conditions.push(format!(
                "created_at <= to_timestamp(${0}/1000.0)",
                param_count
            ));
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

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_count,
            param_count + 1
        ));

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
                branch, commit_sha, files, status, reason, metadata, client_version, created_at
            )
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10::jsonb, $11, $12, $13::jsonb, $14, to_timestamp($15::bigint / 1000.0))
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
        .bind(event.created_at)
        .execute(&self.pool)
        .await;

        match result {
            Ok(res) if res.rows_affected() == 0 => Err(DbError::Duplicate(format!(
                "event_uuid: {}",
                event.event_uuid
            ))),
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => Err(DbError::Duplicate(format!(
                "event_uuid: {}",
                event.event_uuid
            ))),
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn insert_client_events_batch(
        &self,
        events: &[ClientEvent],
    ) -> Result<ClientEventResponse, DbError> {
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
        struct PreparedBatchEvent<'a> {
            id: uuid::Uuid,
            org_id: Option<uuid::Uuid>,
            repo_id: Option<uuid::Uuid>,
            files_json: serde_json::Value,
            created_at: chrono::DateTime<chrono::Utc>,
            event: &'a ClientEvent,
        }

        let mut accepted = Vec::new();
        let mut duplicates = Vec::new();
        let mut errors = Vec::new();
        let mut prepared_events: Vec<PreparedBatchEvent<'_>> = Vec::with_capacity(events.len());

        for event in events {
            let id = match uuid::Uuid::parse_str(&event.id) {
                Ok(id) => id,
                Err(e) => {
                    errors.push(EventError {
                        event_uuid: event.event_uuid.clone(),
                        error: DbError::SerializationError(format!(
                            "invalid event id uuid '{}': {}",
                            event.id, e
                        ))
                        .to_string(),
                    });
                    continue;
                }
            };

            let org_id = match event.org_id.as_deref() {
                Some(raw) => match uuid::Uuid::parse_str(raw) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        errors.push(EventError {
                            event_uuid: event.event_uuid.clone(),
                            error: DbError::SerializationError(format!(
                                "invalid org_id uuid '{}': {}",
                                raw, e
                            ))
                            .to_string(),
                        });
                        continue;
                    }
                },
                None => None,
            };

            let repo_id = match event.repo_id.as_deref() {
                Some(raw) => match uuid::Uuid::parse_str(raw) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        errors.push(EventError {
                            event_uuid: event.event_uuid.clone(),
                            error: DbError::SerializationError(format!(
                                "invalid repo_id uuid '{}': {}",
                                raw, e
                            ))
                            .to_string(),
                        });
                        continue;
                    }
                },
                None => None,
            };

            let files_json = match serde_json::to_value(&event.files) {
                Ok(json) => json,
                Err(e) => {
                    errors.push(EventError {
                        event_uuid: event.event_uuid.clone(),
                        error: DbError::SerializationError(e.to_string()).to_string(),
                    });
                    continue;
                }
            };

            let created_at =
                match chrono::DateTime::<chrono::Utc>::from_timestamp_millis(event.created_at) {
                    Some(ts) => ts,
                    None => {
                        errors.push(EventError {
                            event_uuid: event.event_uuid.clone(),
                            error: DbError::SerializationError(format!(
                                "invalid created_at timestamp millis '{}'",
                                event.created_at
                            ))
                            .to_string(),
                        });
                        continue;
                    }
                };

            prepared_events.push(PreparedBatchEvent {
                id,
                org_id,
                repo_id,
                files_json,
                created_at,
                event,
            });
        }

        if !prepared_events.is_empty() {
            let mut query_builder: QueryBuilder<'_, sqlx::Postgres> = QueryBuilder::new(
                r#"
                INSERT INTO client_events (
                    id, org_id, repo_id, event_uuid, event_type, user_login, user_name,
                    branch, commit_sha, files, status, reason, metadata, client_version, created_at
                )
                "#,
            );

            query_builder.push_values(&prepared_events, |mut builder, row| {
                builder
                    .push_bind(row.id)
                    .push_bind(row.org_id)
                    .push_bind(row.repo_id)
                    .push_bind(&row.event.event_uuid)
                    .push_bind(row.event.event_type.as_str())
                    .push_bind(&row.event.user_login)
                    .push_bind(&row.event.user_name)
                    .push_bind(&row.event.branch)
                    .push_bind(&row.event.commit_sha)
                    .push_bind(&row.files_json)
                    .push_bind(row.event.status.as_str())
                    .push_bind(&row.event.reason)
                    .push_bind(&row.event.metadata)
                    .push_bind(&row.event.client_version)
                    .push_bind(row.created_at);
            });

            query_builder.push(" ON CONFLICT (event_uuid) DO NOTHING RETURNING event_uuid");

            let inserted_event_uuids = match query_builder
                .build_query_scalar::<String>()
                .fetch_all(&self.pool)
                .await
            {
                Ok(rows) => rows,
                Err(e) => return Err(DbError::DatabaseError(e.to_string())),
            };

            let inserted_set: HashSet<&str> =
                inserted_event_uuids.iter().map(String::as_str).collect();
            for row in &prepared_events {
                if inserted_set.contains(row.event.event_uuid.as_str()) {
                    accepted.push(row.event.event_uuid.clone());
                } else {
                    duplicates.push(row.event.event_uuid.clone());
                }
            }
        }

        Ok(ClientEventResponse {
            accepted,
            duplicates,
            errors,
        })
    }

    pub async fn get_client_events(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<ClientEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
        let offset = filter.offset as i64;

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name)
                .await?
                .map(|r| r.id)
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
            conditions.push(format!(
                "created_at >= to_timestamp(${0}/1000.0)",
                param_count
            ));
            param_count += 1;
        }
        if filter.end_date.is_some() {
            conditions.push(format!(
                "created_at <= to_timestamp(${0}/1000.0)",
                param_count
            ));
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

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_count,
            param_count + 1
        ));

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
                    metadata: serde_json::from_str(&metadata_json)
                        .unwrap_or(serde_json::Value::Null),
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

    pub async fn get_combined_events(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<CombinedEvent>, DbError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit } as i32;
        let offset = filter.offset as i32;

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            // Fallback: handler may have set filter.org_id directly (UUID) to avoid a DB roundtrip.
            filter.org_id.clone()
        };

        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name)
                .await?
                .map(|r| r.id)
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
        let before_created_at = filter
            .before_created_at
            .and_then(chrono::DateTime::from_timestamp_millis);

        // Fast path: skip the expensive UNION ALL with github_events when
        // the caller does not explicitly request source='github'.
        let use_client_only_fast_path =
            filter.source.as_deref() != Some("github");

        let result = if use_client_only_fast_path {
            sqlx::query(
                r#"
                SELECT
                    c.id::TEXT AS id,
                    'client'::TEXT AS source,
                    c.event_type,
                    c.created_at,
                    COALESCE(ica.canonical_login, c.user_login) AS user_login,
                    r.full_name AS repo_name,
                    c.branch,
                    c.status,
                    jsonb_strip_nulls(
                        jsonb_build_object(
                            'reason', c.reason,
                            'files', c.files,
                            'event_uuid', c.event_uuid,
                            'commit_sha', c.commit_sha,
                            'user_name', c.user_name
                        )
                        || CASE
                            WHEN jsonb_typeof(COALESCE(c.metadata, '{}'::jsonb)) = 'object'
                                THEN COALESCE(c.metadata, '{}'::jsonb)
                            ELSE jsonb_build_object('metadata', COALESCE(c.metadata, 'null'::jsonb))
                        END
                    ) AS details
                FROM client_events c
                LEFT JOIN repos r ON c.repo_id = r.id
                LEFT JOIN identity_aliases ica
                  ON ica.alias_login = c.user_login
                 AND ($1::uuid IS NULL OR ica.org_id = $1::uuid)
                WHERE ($1::uuid IS NULL OR c.org_id = $1::uuid)
                  AND ($2::uuid IS NULL OR c.repo_id = $2::uuid)
                  AND ($4::text IS NULL OR c.event_type = $4)
                  AND ($5::text IS NULL OR c.user_login = $5 OR COALESCE(ica.canonical_login, c.user_login) = $5)
                  AND ($6::text IS NULL OR c.branch = $6)
                  AND ($7::timestamptz IS NULL OR c.created_at >= $7)
                  AND ($8::timestamptz IS NULL OR c.created_at <= $8)
                  AND ($9::text IS NULL OR c.status = $9)
                  AND (
                      $12::timestamptz IS NULL
                      OR c.created_at < $12
                      OR ($13::text IS NOT NULL AND c.created_at = $12 AND c.id::text < $13::text)
                  )
                ORDER BY c.created_at DESC, c.id DESC
                LIMIT $10 OFFSET $11
                "#
            )
            .bind(&org_id)          // $1
            .bind(&repo_id)         // $2
            .bind(&filter.source)   // $3 (unused in fast path but keeps bind order)
            .bind(&filter.event_type) // $4
            .bind(&filter.user_login) // $5
            .bind(&filter.branch)   // $6
            .bind(start_date)       // $7
            .bind(end_date)         // $8
            .bind(&filter.status)   // $9
            .bind(limit)            // $10
            .bind(offset)           // $11
            .bind(before_created_at) // $12
            .bind(&filter.before_id) // $13
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?
        } else {
            sqlx::query(
            r#"
            SELECT id, source, event_type, created_at, user_login, repo_name, branch, status, details
            FROM (
                SELECT
                    g.id::TEXT AS id,
                    'github'::TEXT AS source,
                    g.event_type,
                    g.created_at,
                    COALESCE(iga.canonical_login, g.actor_login) AS user_login,
                    r.full_name AS repo_name,
                    g.ref_name AS branch,
                    NULL::TEXT AS status,
                    jsonb_build_object(
                        'commits_count', g.commits_count,
                        'after_sha', g.after_sha
                    ) AS details
                FROM github_events g
                LEFT JOIN repos r ON g.repo_id = r.id
                LEFT JOIN identity_aliases iga
                  ON iga.alias_login = g.actor_login
                 AND ($1::uuid IS NULL OR iga.org_id = $1::uuid)
                WHERE ($1::uuid IS NULL OR g.org_id = $1::uuid)
                  AND ($2::uuid IS NULL OR g.repo_id = $2::uuid)
                  AND ($3::text IS NULL OR $3 = 'github')
                  AND ($4::text IS NULL OR g.event_type = $4)
                  AND ($5::text IS NULL OR g.actor_login = $5 OR COALESCE(iga.canonical_login, g.actor_login) = $5)
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
                    COALESCE(ica.canonical_login, c.user_login) AS user_login,
                    r.full_name AS repo_name,
                    c.branch,
                    c.status,
                    jsonb_strip_nulls(
                        jsonb_build_object(
                            'reason', c.reason,
                            'files', c.files,
                            'event_uuid', c.event_uuid,
                            'commit_sha', c.commit_sha,
                            'user_name', c.user_name
                        )
                        || CASE
                            WHEN jsonb_typeof(COALESCE(c.metadata, '{}'::jsonb)) = 'object'
                                THEN COALESCE(c.metadata, '{}'::jsonb)
                            ELSE jsonb_build_object('metadata', COALESCE(c.metadata, 'null'::jsonb))
                        END
                    ) AS details
                FROM client_events c
                LEFT JOIN repos r ON c.repo_id = r.id
                LEFT JOIN identity_aliases ica
                  ON ica.alias_login = c.user_login
                 AND ($1::uuid IS NULL OR ica.org_id = $1::uuid)
                WHERE ($1::uuid IS NULL OR c.org_id = $1::uuid)
                  AND ($2::uuid IS NULL OR c.repo_id = $2::uuid)
                  AND ($3::text IS NULL OR $3 = 'client')
                  AND ($4::text IS NULL OR c.event_type = $4)
                  AND ($5::text IS NULL OR c.user_login = $5 OR COALESCE(ica.canonical_login, c.user_login) = $5)
                  AND ($6::text IS NULL OR c.branch = $6)
                  AND ($7::timestamptz IS NULL OR c.created_at >= $7)
                  AND ($8::timestamptz IS NULL OR c.created_at <= $8)
                  AND ($9::text IS NULL OR c.status = $9)
            ) combined
            WHERE (
                $12::timestamptz IS NULL
                OR combined.created_at < $12
                OR ($13::text IS NOT NULL AND combined.created_at = $12 AND combined.id < $13::text)
            )
            ORDER BY created_at DESC, id DESC
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
        .bind(before_created_at)
        .bind(&filter.before_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?
        };

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

    /// Same as get_combined_events but without the 100-record default cap.
    /// Used for compliance exports — returns up to 50,000 records.
    pub async fn get_events_for_export(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<CombinedEvent>, DbError> {
        let limit = if filter.limit == 0 {
            50_000_i32
        } else {
            filter.limit.min(50_000) as i32
        };
        let export_filter = EventFilter {
            limit: limit as usize,
            offset: 0,
            ..filter.clone()
        };
        self.get_combined_events(&export_filter).await
    }

    /// Export helper for policy drift audit events.
    /// Applies the same date/user/repo/org scoping model used in compliance exports.
    pub async fn get_policy_drift_events_for_export(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<crate::models::PolicyDriftEventRecord>, DbError> {
        let limit = if filter.limit == 0 {
            50_000_i64
        } else {
            (filter.limit.min(50_000)) as i64
        };
        let start_date = filter
            .start_date
            .and_then(chrono::DateTime::from_timestamp_millis);
        let end_date = filter
            .end_date
            .and_then(chrono::DateTime::from_timestamp_millis);

        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                user_login,
                action,
                repo_name,
                result,
                before_checksum,
                after_checksum,
                duration_ms,
                COALESCE(metadata, '{}'::jsonb) AS metadata,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM policy_drift_events
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
              AND ($2::text IS NULL OR user_login = $2)
              AND ($3::text IS NULL OR repo_name = $3)
              AND ($4::timestamptz IS NULL OR created_at >= $4)
              AND ($5::timestamptz IS NULL OR created_at <= $5)
            ORDER BY created_at DESC
            LIMIT $6
            "#,
        )
        .bind(filter.org_id.as_deref())
        .bind(filter.user_login.as_deref())
        .bind(filter.repo_full_name.as_deref())
        .bind(start_date)
        .bind(end_date)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let records = rows
            .iter()
            .map(|row| crate::models::PolicyDriftEventRecord {
                id: row.get("id"),
                org_id: row.get("org_id"),
                user_login: row.get("user_login"),
                action: row.get("action"),
                repo_name: row.get("repo_name"),
                result: row.get("result"),
                before_checksum: row.get("before_checksum"),
                after_checksum: row.get("after_checksum"),
                duration_ms: row.get("duration_ms"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at_ms"),
            })
            .collect();

        Ok(records)
    }

    /// Export helper for policy change requests (request/approve/reject workflow).
    /// Applies date/user/repo/org scoping compatible with compliance exports.
    pub async fn get_policy_change_requests_for_export(
        &self,
        filter: &EventFilter,
    ) -> Result<Vec<crate::models::PolicyChangeRequestRecord>, DbError> {
        let limit = if filter.limit == 0 {
            50_000_i64
        } else {
            (filter.limit.min(50_000)) as i64
        };
        let start_date = filter
            .start_date
            .and_then(chrono::DateTime::from_timestamp_millis);
        let end_date = filter
            .end_date
            .and_then(chrono::DateTime::from_timestamp_millis);

        let rows = sqlx::query(
            r#"
            SELECT
                r.id::text AS id,
                r.org_id::text AS org_id,
                r.repo_id::text AS repo_id,
                r.repo_name AS repo_name,
                r.requested_by AS requested_by,
                r.requested_checksum AS requested_checksum,
                COALESCE(r.requested_config, '{}'::jsonb) AS requested_config,
                r.reason AS reason,
                COALESCE(d.decision, 'pending') AS status,
                d.decided_by AS decided_by,
                d.note AS decision_note,
                EXTRACT(EPOCH FROM r.created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM d.created_at)::bigint * 1000 AS decided_at_ms
            FROM policy_change_requests r
            LEFT JOIN policy_change_request_decisions d
              ON d.request_id = r.id
            WHERE ($1::uuid IS NULL OR r.org_id = $1::uuid)
              AND ($2::text IS NULL OR r.requested_by = $2)
              AND ($3::text IS NULL OR r.repo_name = $3)
              AND ($4::timestamptz IS NULL OR r.created_at >= $4)
              AND ($5::timestamptz IS NULL OR r.created_at <= $5)
            ORDER BY r.created_at DESC
            LIMIT $6
            "#,
        )
        .bind(filter.org_id.as_deref())
        .bind(filter.user_login.as_deref())
        .bind(filter.repo_full_name.as_deref())
        .bind(start_date)
        .bind(end_date)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let records = rows
            .iter()
            .map(|row| {
                let config: serde_json::Value = row.get("requested_config");
                crate::models::PolicyChangeRequestRecord {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    repo_name: row.get("repo_name"),
                    requested_by: row.get("requested_by"),
                    requested_checksum: row.get("requested_checksum"),
                    requested_config: serde_json::from_value(config).unwrap_or_default(),
                    reason: row.get("reason"),
                    status: row.get("status"),
                    decided_by: row.get("decided_by"),
                    decision_note: row.get("decision_note"),
                    created_at: row.get("created_at_ms"),
                    decided_at: row.get("decided_at_ms"),
                }
            })
            .collect();

        Ok(records)
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

        let ingested_at =
            chrono::DateTime::from_timestamp_millis(event.ingested_at).ok_or_else(|| {
                DbError::SerializationError("Invalid ingested_at timestamp".to_string())
            })?;

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
        .bind(event.duration_ms)
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

    pub async fn get_jenkins_integration_status(
        &self,
    ) -> Result<JenkinsIntegrationStatusResponse, DbError> {
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
        let ingested_at =
            chrono::DateTime::from_timestamp_millis(ticket.ingested_at).ok_or_else(|| {
                DbError::SerializationError("Invalid ingested_at timestamp".to_string())
            })?;
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

    pub async fn get_jira_integration_status(
        &self,
    ) -> Result<JiraIntegrationStatusResponse, DbError> {
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
            .ok_or_else(|| {
                DbError::SerializationError("Invalid created_at timestamp".to_string())
            })?;

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
        self.append_project_ticket_relations_full(ticket_id, commit_sha, branch, None)
            .await
    }

    pub async fn append_project_ticket_relations_full(
        &self,
        ticket_id: &str,
        commit_sha: Option<&str>,
        branch: Option<&str>,
        pr_ref: Option<&str>,
    ) -> Result<bool, DbError> {
        let commit_sha = commit_sha.map(str::trim).filter(|s| !s.is_empty());
        let branch = branch.map(str::trim).filter(|s| !s.is_empty());
        let pr_ref = pr_ref.map(str::trim).filter(|s| !s.is_empty());

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
              related_prs = CASE
                WHEN $4::text IS NULL THEN related_prs
                ELSE (
                  SELECT COALESCE(array_agg(DISTINCT x), '{}'::text[])
                  FROM unnest(COALESCE(related_prs, '{}'::text[]) || ARRAY[$4::text]) AS x
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
        .bind(pr_ref)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// Find PRs whose head_sha matches any of the given commit SHAs,
    /// or whose pr_title contains any of the given ticket IDs.
    /// Returns Vec<(pr_number, pr_title, head_sha, repo_full_name)>.
    pub async fn find_prs_related_to_tickets(
        &self,
        commit_shas: &[String],
        ticket_ids: &[String],
        hours: i64,
    ) -> Result<Vec<(i32, Option<String>, Option<String>, Option<String>)>, DbError> {
        if commit_shas.is_empty() && ticket_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query(
            r#"
            SELECT DISTINCT
                prm.pr_number,
                prm.pr_title,
                prm.head_sha,
                r.full_name AS repo_full_name
            FROM pull_request_merges prm
            LEFT JOIN repos r ON r.id = prm.repo_id
            WHERE prm.created_at >= NOW() - make_interval(hours => $3::int)
              AND (
                ($1::text[] IS NOT NULL AND prm.head_sha = ANY($1::text[]))
                OR ($2::text[] IS NOT NULL AND EXISTS (
                  SELECT 1 FROM unnest($2::text[]) AS tid
                  WHERE UPPER(prm.pr_title) LIKE '%' || UPPER(tid) || '%'
                ))
              )
            ORDER BY prm.pr_number
            LIMIT 200
            "#,
        )
        .bind(commit_shas)
        .bind(ticket_ids)
        .bind(hours as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get::<i32, _>("pr_number"),
                    row.get::<Option<String>, _>("pr_title"),
                    row.get::<Option<String>, _>("head_sha"),
                    row.get::<Option<String>, _>("repo_full_name"),
                )
            })
            .collect())
    }

    pub async fn get_recent_commit_events_for_ticket_correlation(
        &self,
        org_name: Option<&str>,
        repo_full_name: Option<&str>,
        hours: i64,
        limit: i64,
    ) -> Result<
        Vec<(
            String,
            Option<String>,
            Option<String>,
            serde_json::Value,
            Option<String>,
        )>,
        DbError,
    > {
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
            self.get_repo_by_full_name(repo_full_name)
                .await?
                .map(|r| r.id)
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

                let pipeline =
                    row.get::<Option<String>, _>("pipeline_event_id")
                        .map(|pipeline_event_id| {
                            let ingested_at = row
                                .get::<Option<chrono::DateTime<chrono::Utc>>, _>(
                                    "pipeline_ingested_at",
                                )
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

    pub async fn get_ticket_flow_correlations_v2(
        &self,
        filter: &CorrelationV2Query,
    ) -> Result<(Vec<TicketFlowCorrelation>, i64), DbError> {
        let limit = if filter.limit == 0 {
            50
        } else {
            filter.limit.min(500)
        } as i64;
        let offset = filter.offset as i64;
        let hours = filter.hours.unwrap_or(24 * 7).clamp(1, 24 * 90);

        let org_id = if let Some(org_name) = filter.org_name.as_deref() {
            self.get_org_by_login(org_name).await?.map(|o| o.id)
        } else {
            None
        };
        let repo_id = if let Some(repo_full_name) = filter.repo_full_name.as_deref() {
            self.get_repo_by_full_name(repo_full_name)
                .await?
                .map(|r| r.id)
        } else {
            None
        };
        let ticket_id = filter
            .ticket_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_uppercase());

        if filter.org_name.is_some() && org_id.is_none() {
            return Ok((vec![], 0));
        }
        if filter.repo_full_name.is_some() && repo_id.is_none() {
            return Ok((vec![], 0));
        }

        let count_row = sqlx::query(
            r#"
            WITH base AS (
                SELECT
                    ct.ticket_id,
                    ct.commit_sha,
                    COALESCE(c.created_at, ct.created_at) AS ordering_ts
                FROM commit_ticket_correlations ct
                LEFT JOIN project_tickets pt
                  ON pt.ticket_id = ct.ticket_id
                 AND (ct.org_id IS NULL OR pt.org_id = ct.org_id)
                LEFT JOIN LATERAL (
                    SELECT c.created_at, c.repo_id
                    FROM client_events c
                    WHERE c.event_type = 'commit'
                      AND c.commit_sha = ct.commit_sha
                    ORDER BY c.created_at DESC
                    LIMIT 1
                ) c ON TRUE
                WHERE ($1::uuid IS NULL OR ct.org_id = $1::uuid OR pt.org_id = $1::uuid)
                  AND ($2::uuid IS NULL OR c.repo_id = $2::uuid)
                  AND ($3::text IS NULL OR ct.ticket_id = $3)
                  AND COALESCE(c.created_at, ct.created_at) >= NOW() - make_interval(hours => $4::int)
            )
            SELECT COUNT(*)::bigint AS total
            FROM base
            "#,
        )
        .bind(&org_id)
        .bind(&repo_id)
        .bind(&ticket_id)
        .bind(hours as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");
        if total == 0 {
            return Ok((vec![], 0));
        }

        let rows = sqlx::query(
            r#"
            SELECT
                ct.ticket_id,
                pt.status AS ticket_status,
                ct.correlation_source,
                ct.confidence AS correlation_confidence,
                ct.commit_sha,
                c.branch,
                c.user_login,
                r.full_name AS repo_name,
                CASE
                    WHEN c.created_at IS NULL THEN NULL
                    ELSE EXTRACT(EPOCH FROM c.created_at)::bigint * 1000
                END AS commit_created_at_ms,
                p.id::text AS pipeline_event_id,
                p.pipeline_id,
                p.job_name,
                p.status AS pipeline_status,
                p.duration_ms AS pipeline_duration_ms,
                p.triggered_by,
                CASE
                    WHEN p.ingested_at IS NULL THEN NULL
                    ELSE EXTRACT(EPOCH FROM p.ingested_at)::bigint * 1000
                END AS pipeline_ingested_at_ms
            FROM commit_ticket_correlations ct
            LEFT JOIN project_tickets pt
              ON pt.ticket_id = ct.ticket_id
             AND (ct.org_id IS NULL OR pt.org_id = ct.org_id)
            LEFT JOIN LATERAL (
                SELECT c.branch, c.user_login, c.repo_id, c.created_at
                FROM client_events c
                WHERE c.event_type = 'commit'
                  AND c.commit_sha = ct.commit_sha
                ORDER BY c.created_at DESC
                LIMIT 1
            ) c ON TRUE
            LEFT JOIN repos r ON r.id = c.repo_id
            LEFT JOIN LATERAL (
                SELECT pe.*
                FROM pipeline_events pe
                WHERE pe.commit_sha IS NOT NULL
                  AND (
                    pe.commit_sha = ct.commit_sha
                    OR pe.commit_sha LIKE ct.commit_sha || '%'
                    OR ct.commit_sha LIKE pe.commit_sha || '%'
                  )
                ORDER BY pe.ingested_at DESC
                LIMIT 1
            ) p ON TRUE
            WHERE ($1::uuid IS NULL OR ct.org_id = $1::uuid OR pt.org_id = $1::uuid)
              AND ($2::uuid IS NULL OR c.repo_id = $2::uuid)
              AND ($3::text IS NULL OR ct.ticket_id = $3)
              AND COALESCE(c.created_at, ct.created_at) >= NOW() - make_interval(hours => $4::int)
            ORDER BY COALESCE(c.created_at, ct.created_at) DESC, ct.ticket_id, ct.commit_sha
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(&org_id)
        .bind(&repo_id)
        .bind(&ticket_id)
        .bind(hours as i32)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let items = rows
            .into_iter()
            .map(|row| {
                let pipeline =
                    row.get::<Option<String>, _>("pipeline_event_id")
                        .map(|pipeline_event_id| CommitPipelineRun {
                            pipeline_event_id,
                            pipeline_id: row.get("pipeline_id"),
                            job_name: row.get("job_name"),
                            status: row.get("pipeline_status"),
                            duration_ms: row.get("pipeline_duration_ms"),
                            triggered_by: row.get("triggered_by"),
                            ingested_at: row
                                .get::<Option<i64>, _>("pipeline_ingested_at_ms")
                                .unwrap_or_default(),
                        });

                TicketFlowCorrelation {
                    ticket_id: row.get("ticket_id"),
                    ticket_status: row.get("ticket_status"),
                    correlation_source: row.get("correlation_source"),
                    correlation_confidence: row.get("correlation_confidence"),
                    commit_sha: row.get("commit_sha"),
                    branch: row.get("branch"),
                    user_login: row.get("user_login"),
                    repo_name: row.get("repo_name"),
                    commit_created_at: row.get("commit_created_at_ms"),
                    pipeline,
                }
            })
            .collect();

        Ok((items, total))
    }

    pub async fn get_pipeline_health_stats(
        &self,
        org_id: Option<&str>,
    ) -> Result<PipelineHealthStats, DbError> {
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

    pub async fn get_daily_activity(
        &self,
        org_id: Option<&str>,
        days: i64,
    ) -> Result<Vec<DailyActivityPoint>, DbError> {
        let rows = sqlx::query(
            r#"
            WITH series AS (
              SELECT generate_series(
                (date_trunc('day', NOW() AT TIME ZONE 'UTC') - (($1::int - 1) * INTERVAL '1 day')),
                date_trunc('day', NOW() AT TIME ZONE 'UTC'),
                INTERVAL '1 day'
              )::date AS day_utc
            )
            SELECT
              to_char(s.day_utc, 'YYYY-MM-DD') AS day,
              COALESCE(SUM(CASE WHEN ce.event_type = 'commit' THEN 1 ELSE 0 END), 0)::bigint AS commits,
              COALESCE(SUM(CASE WHEN ce.event_type = 'successful_push' THEN 1 ELSE 0 END), 0)::bigint AS pushes
            FROM series s
            LEFT JOIN client_events ce
              ON ce.created_at >= s.day_utc::timestamp
             AND ce.created_at < (s.day_utc::timestamp + INTERVAL '1 day')
             AND ($2::uuid IS NULL OR ce.org_id = $2::uuid)
            GROUP BY s.day_utc
            ORDER BY s.day_utc DESC
            "#,
        )
        .bind(days as i32)
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let points = rows
            .into_iter()
            .map(|row| DailyActivityPoint {
                day: row.get("day"),
                commits: row.get("commits"),
                pushes: row.get("pushes"),
            })
            .collect();

        Ok(points)
    }

    // ========================================================================
    // POLICIES
    // ========================================================================

    pub async fn save_policy(
        &self,
        repo_id: &str,
        config: &GitGovConfig,
        checksum: &str,
        override_actor: &str,
    ) -> Result<(), DbError> {
        let config_json =
            serde_json::to_value(config).map_err(|e| DbError::SerializationError(e.to_string()))?;

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

    pub async fn mark_webhook_processed(
        &self,
        id: &str,
        error: Option<&str>,
    ) -> Result<(), DbError> {
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

    pub async fn validate_api_key(&self, key_hash: &str) -> Result<ApiKeyAuthValidation, DbError> {
        if let Some(cached) = self.get_cached_api_key_auth(key_hash) {
            return Ok(ApiKeyAuthValidation {
                auth: cached,
                used_stale_cache: false,
            });
        }

        let simulated_auth_db_failure = auth_db_failure_simulation_enabled();
        if simulated_auth_db_failure {
            tracing::warn!(
                "Simulating auth DB query failure via debug failpoint (validate_api_key)"
            );
        }

        let result = if simulated_auth_db_failure {
            Err("Simulated auth DB failure (debug failpoint)".to_string())
        } else {
            sqlx::query(
                r#"
                SELECT client_id, role, org_id::text, last_used
                FROM api_keys
                WHERE key_hash = $1 AND is_active = TRUE
                "#,
            )
            .bind(key_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())
        };

        let result = match result {
            Ok(row) => row,
            Err(error_msg) => {
                let (failure_streak, should_fail_closed) = self.note_auth_db_failure();
                if should_fail_closed {
                    tracing::warn!(
                        error = %error_msg,
                        failure_streak,
                        fail_closed_threshold = self.auth_stale_fail_closed_after,
                        "Auth DB failure threshold reached; stale auth fallback disabled"
                    );
                    return Err(DbError::DatabaseError(error_msg));
                }
                if let Some((stale_auth, stale_age_secs)) =
                    self.get_stale_cached_api_key_auth(key_hash)
                {
                    tracing::warn!(
                        error = %error_msg,
                        client_id = %stale_auth.0,
                        failure_streak,
                        stale_age_secs,
                        "Using stale API key auth cache due transient database error"
                    );
                    return Ok(ApiKeyAuthValidation {
                        auth: Some(stale_auth),
                        used_stale_cache: true,
                    });
                }
                return Err(DbError::DatabaseError(error_msg));
            }
        };

        self.reset_auth_db_failure_streak();

        match result {
            Some(row) => {
                let role: String = row.get("role");
                let role = UserRole::from_str(&role);
                let client_id: String = row.get("client_id");
                let org_id: Option<String> = row.get("org_id");
                let auth_tuple = Some((client_id.clone(), role.clone(), org_id.clone()));

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

                self.put_cached_api_key_auth(key_hash, auth_tuple.clone());
                Ok(ApiKeyAuthValidation {
                    auth: auth_tuple,
                    used_stale_cache: false,
                })
            }
            None => {
                // Cache negative lookups briefly to reduce repeated DB hits on invalid tokens.
                self.put_cached_api_key_auth(key_hash, None);
                Ok(ApiKeyAuthValidation {
                    auth: None,
                    used_stale_cache: false,
                })
            }
        }
    }

    pub async fn create_api_key(
        &self,
        key_hash: &str,
        client_id: &str,
        org_id: Option<&str>,
        role: &UserRole,
    ) -> Result<(), DbError> {
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

        self.invalidate_auth_cache_key(key_hash);

        Ok(())
    }

    pub async fn ensure_admin_api_key(
        &self,
        key_hash: &str,
        client_id: &str,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO api_keys (key_hash, client_id, org_id, role, is_active)
            VALUES ($1, $2, NULL, $3, TRUE)
            ON CONFLICT (key_hash) DO UPDATE
            SET
                client_id = EXCLUDED.client_id,
                org_id = NULL,
                role = EXCLUDED.role,
                is_active = TRUE
            "#,
        )
        .bind(key_hash)
        .bind(client_id)
        .bind(UserRole::Admin.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        self.invalidate_auth_cache_key(key_hash);

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

        let revoked = result.rows_affected() > 0;
        if revoked {
            // Revoke works by key id; safest is clearing auth cache to avoid stale entries.
            self.invalidate_auth_cache_all();
        }

        Ok(revoked)
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
            Ok(res) if res.rows_affected() == 0 => Err(DbError::Duplicate(format!(
                "delivery_id: {}",
                record.delivery_id
            ))),
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => Err(DbError::Duplicate(format!(
                "delivery_id: {}",
                record.delivery_id
            ))),
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

        self.create_api_key(&key_hash, client_id, None, &UserRole::Admin)
            .await?;

        Ok(Some(api_key))
    }

    // ========================================================================
    // HEALTH CHECK
    // ========================================================================

    pub async fn health_check(&self) -> Result<(bool, i64), DbError> {
        let result =
            sqlx::query("SELECT COUNT(*) as count FROM client_events WHERE status = 'pending'")
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
        filter: &NoncomplianceSignalsQuery<'_>,
    ) -> Result<(Vec<NoncomplianceSignal>, i64), DbError> {
        let mut conditions = Vec::new();
        let mut param_count = 1;

        if filter.org_id.is_some() {
            conditions.push(format!("ns.org_id = ${}::uuid", param_count));
            param_count += 1;
        }

        if filter.confidence.is_some() {
            conditions.push(format!("ns.confidence = ${}", param_count));
            param_count += 1;
        }
        if filter.status.is_some() {
            conditions.push(format!(
                "COALESCE((SELECT sd.decision FROM signal_decisions sd WHERE sd.signal_id = ns.id ORDER BY sd.created_at DESC LIMIT 1), ns.status) = ${}",
                param_count
            ));
            param_count += 1;
        }
        if filter.signal_type.is_some() {
            conditions.push(format!("ns.signal_type = ${}", param_count));
            param_count += 1;
        }
        if filter.actor_login.is_some() {
            conditions.push(format!("ns.actor_login = ${}", param_count));
            param_count += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let count_query = format!(
            "SELECT COUNT(*) as total FROM noncompliance_signals ns{}",
            where_clause
        );

        let mut count_sql = sqlx::query(&count_query);

        if let Some(org) = filter.org_id {
            count_sql = count_sql.bind(org);
        }
        if let Some(c) = filter.confidence {
            count_sql = count_sql.bind(c);
        }
        if let Some(s) = filter.status {
            count_sql = count_sql.bind(s);
        }
        if let Some(st) = filter.signal_type {
            count_sql = count_sql.bind(st);
        }
        if let Some(actor) = filter.actor_login {
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

        if let Some(org) = filter.org_id {
            data_sql = data_sql.bind(org);
        }
        if let Some(c) = filter.confidence {
            data_sql = data_sql.bind(c);
        }
        if let Some(s) = filter.status {
            data_sql = data_sql.bind(s);
        }
        if let Some(st) = filter.signal_type {
            data_sql = data_sql.bind(st);
        }
        if let Some(actor) = filter.actor_login {
            data_sql = data_sql.bind(actor);
        }
        data_sql = data_sql.bind(filter.limit).bind(filter.offset);

        let rows = data_sql
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let signals: Vec<NoncomplianceSignal> = rows
            .iter()
            .map(|row| {
                let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
                let investigated_at: Option<chrono::DateTime<chrono::Utc>> =
                    row.get("investigated_at");

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
        .bind(uuid::Uuid::new_v4().to_string())
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
                if insert_err_msg.contains("signal_decisions")
                    && insert_err_msg.contains("does not exist")
                {
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

    pub async fn get_signal_by_id(
        &self,
        signal_id: &str,
    ) -> Result<Option<NoncomplianceSignal>, DbError> {
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
                let investigated_at: Option<chrono::DateTime<chrono::Utc>> =
                    row.get("investigated_at");

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
        let signal = self
            .get_signal_by_id(signal_id)
            .await?
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
        .bind(uuid::Uuid::new_v4().to_string())
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

    async fn detect_v2_commit_no_ticket_signals(
        &self,
        org_id: &str,
        hours: i64,
    ) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            WITH latest_commits AS (
                SELECT DISTINCT ON (c.commit_sha)
                    c.id,
                    c.org_id,
                    c.repo_id,
                    c.user_login,
                    c.branch,
                    c.commit_sha,
                    c.created_at,
                    COALESCE(r.full_name, c.metadata->>'repo_name') AS repo_name
                FROM client_events c
                LEFT JOIN repos r ON r.id = c.repo_id
                WHERE c.org_id = $1::uuid
                  AND c.event_type = 'commit'
                  AND c.commit_sha IS NOT NULL
                  AND c.commit_sha <> ''
                  AND c.created_at >= NOW() - make_interval(hours => $2::int)
                ORDER BY c.commit_sha, c.created_at DESC
            ),
            candidates AS (
                SELECT lc.*
                FROM latest_commits lc
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM commit_ticket_correlations ct
                    WHERE ct.commit_sha = lc.commit_sha
                      AND (ct.org_id = lc.org_id OR ct.org_id IS NULL)
                )
                  AND (
                    lc.branch IN ('main', 'master')
                    OR EXISTS (
                        SELECT 1
                        FROM policies p
                        WHERE p.repo_id = lc.repo_id
                          AND jsonb_typeof(p.config->'branches'->'protected') = 'array'
                          AND (p.config->'branches'->'protected') ? lc.branch
                    )
                  )
                  AND NOT EXISTS (
                    SELECT 1
                    FROM noncompliance_signals ns
                    WHERE ns.org_id = lc.org_id
                      AND ns.signal_type = 'commit_no_ticket'
                      AND ns.commit_sha = lc.commit_sha
                  )
            )
            INSERT INTO noncompliance_signals (
                org_id,
                repo_id,
                client_event_id,
                signal_type,
                confidence,
                actor_login,
                branch,
                commit_sha,
                evidence,
                context
            )
            SELECT
                c.org_id,
                c.repo_id,
                c.id,
                'commit_no_ticket',
                'medium',
                c.user_login,
                c.branch,
                c.commit_sha,
                jsonb_build_object(
                    'reason', 'Commit on protected branch without linked ticket',
                    'repo_name', c.repo_name,
                    'commit_created_at', EXTRACT(EPOCH FROM c.created_at)::bigint * 1000
                ),
                jsonb_build_object(
                    'detection_window_hours', $2::int,
                    'source', 'v2_minimal'
                )
            FROM candidates c
            "#,
        )
        .bind(org_id)
        .bind(hours as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    async fn detect_v2_ticket_no_coverage_signals(
        &self,
        org_id: &str,
        hours: i64,
    ) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            WITH candidates AS (
                SELECT
                    pt.org_id,
                    pt.ticket_id,
                    pt.status,
                    pt.title,
                    pt.priority,
                    pt.ticket_type,
                    pt.assignee,
                    pt.reporter,
                    COALESCE(pt.updated_at, pt.ingested_at) AS ticket_updated_at
                FROM project_tickets pt
                WHERE pt.org_id = $1::uuid
                  AND COALESCE(pt.updated_at, pt.ingested_at)
                      >= NOW() - make_interval(hours => $2::int)
                  AND (
                    lower(COALESCE(pt.status, '')) IN ('done', 'closed', 'resolved')
                    OR lower(COALESCE(pt.status, '')) LIKE '%done%'
                    OR lower(COALESCE(pt.status, '')) LIKE '%closed%'
                    OR lower(COALESCE(pt.status, '')) LIKE '%resolved%'
                  )
                  AND NOT EXISTS (
                    SELECT 1
                    FROM commit_ticket_correlations ct
                    WHERE ct.ticket_id = pt.ticket_id
                      AND ct.org_id = pt.org_id
                  )
                  AND NOT EXISTS (
                    SELECT 1
                    FROM noncompliance_signals ns
                    WHERE ns.org_id = pt.org_id
                      AND ns.signal_type = 'ticket_no_coverage'
                      AND ns.evidence->>'ticket_id' = pt.ticket_id
                  )
            )
            INSERT INTO noncompliance_signals (
                org_id,
                signal_type,
                confidence,
                actor_login,
                evidence,
                context
            )
            SELECT
                c.org_id,
                'ticket_no_coverage',
                'high',
                COALESCE(NULLIF(c.assignee, ''), NULLIF(c.reporter, ''), 'system'),
                jsonb_build_object(
                    'ticket_id', c.ticket_id,
                    'ticket_status', c.status,
                    'ticket_updated_at', CASE
                        WHEN c.ticket_updated_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM c.ticket_updated_at)::bigint * 1000
                    END,
                    'reason', 'Done ticket without correlated commits'
                ),
                jsonb_build_object(
                    'title', c.title,
                    'priority', c.priority,
                    'ticket_type', c.ticket_type,
                    'detection_window_hours', $2::int,
                    'source', 'v2_minimal'
                )
            FROM candidates c
            "#,
        )
        .bind(org_id)
        .bind(hours as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    async fn detect_v2_pipeline_failure_streak_signals(
        &self,
        org_id: &str,
        hours: i64,
        streak_size: i32,
    ) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            WITH ranked AS (
                SELECT
                    pe.org_id,
                    COALESCE(NULLIF(pe.repo_full_name, ''), '__unknown_repo__') AS repo_name_key,
                    COALESCE(NULLIF(pe.branch, ''), '__unknown_branch__') AS branch_key,
                    pe.status,
                    pe.job_name,
                    pe.triggered_by,
                    pe.id::text AS pipeline_event_id,
                    pe.ingested_at,
                    ROW_NUMBER() OVER (
                        PARTITION BY
                            COALESCE(NULLIF(pe.repo_full_name, ''), '__unknown_repo__'),
                            COALESCE(NULLIF(pe.branch, ''), '__unknown_branch__')
                        ORDER BY pe.ingested_at DESC, pe.id DESC
                    ) AS rn
                FROM pipeline_events pe
                WHERE pe.org_id = $1::uuid
                  AND pe.ingested_at >= NOW() - make_interval(hours => $2::int)
            ),
            streaks AS (
                SELECT
                    r.org_id,
                    r.repo_name_key,
                    r.branch_key,
                    MAX(r.ingested_at) AS latest_ingested_at,
                    (array_agg(r.pipeline_event_id ORDER BY r.ingested_at DESC, r.pipeline_event_id DESC))[1] AS latest_pipeline_event_id,
                    (array_agg(r.job_name ORDER BY r.ingested_at DESC, r.pipeline_event_id DESC))[1] AS latest_job_name,
                    COALESCE(
                        (array_agg(NULLIF(r.triggered_by, '') ORDER BY r.ingested_at DESC, r.pipeline_event_id DESC))[1],
                        'system'
                    ) AS actor_login,
                    array_agg(r.status ORDER BY r.ingested_at DESC, r.pipeline_event_id DESC) AS recent_statuses,
                    COUNT(*)::int AS sample_size
                FROM ranked r
                WHERE r.rn <= $3::int
                GROUP BY r.org_id, r.repo_name_key, r.branch_key
                HAVING COUNT(*) = $3::int
                   AND BOOL_AND(lower(COALESCE(r.status, '')) IN ('failure', 'aborted', 'unstable'))
            ),
            candidates AS (
                SELECT s.*
                FROM streaks s
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM noncompliance_signals ns
                    WHERE ns.org_id = s.org_id
                      AND ns.signal_type = 'pipeline_failure_streak'
                      AND COALESCE(ns.evidence->>'repo_name', '__unknown_repo__') = s.repo_name_key
                      AND COALESCE(ns.evidence->>'branch', '__unknown_branch__') = s.branch_key
                      AND ns.evidence->>'latest_pipeline_event_id' = s.latest_pipeline_event_id
                )
            )
            INSERT INTO noncompliance_signals (
                org_id,
                signal_type,
                confidence,
                actor_login,
                branch,
                evidence,
                context
            )
            SELECT
                c.org_id,
                'pipeline_failure_streak',
                'high',
                c.actor_login,
                NULLIF(c.branch_key, '__unknown_branch__'),
                jsonb_build_object(
                    'repo_name', NULLIF(c.repo_name_key, '__unknown_repo__'),
                    'branch', NULLIF(c.branch_key, '__unknown_branch__'),
                    'latest_pipeline_event_id', c.latest_pipeline_event_id,
                    'latest_job_name', c.latest_job_name,
                    'recent_statuses', c.recent_statuses,
                    'sample_size', c.sample_size,
                    'latest_ingested_at', EXTRACT(EPOCH FROM c.latest_ingested_at)::bigint * 1000,
                    'reason', 'Three or more consecutive failing pipelines on the same branch'
                ),
                jsonb_build_object(
                    'detection_window_hours', $2::int,
                    'streak_size', $3::int,
                    'source', 'v2_advanced'
                )
            FROM candidates c
            "#,
        )
        .bind(org_id)
        .bind(hours as i32)
        .bind(streak_size)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    async fn detect_v2_stale_in_progress_signals(
        &self,
        org_id: &str,
        hours: i64,
        stale_days: i32,
    ) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            WITH ticket_activity AS (
                SELECT
                    pt.org_id,
                    pt.ticket_id,
                    pt.status,
                    pt.title,
                    pt.priority,
                    pt.ticket_type,
                    pt.assignee,
                    pt.reporter,
                    COALESCE(pt.updated_at, pt.ingested_at) AS ticket_updated_at,
                    corr.last_commit_at,
                    COALESCE(corr.commit_links, 0)::bigint AS commit_links,
                    CASE
                        WHEN corr.last_commit_at IS NULL THEN ''
                        ELSE (EXTRACT(EPOCH FROM corr.last_commit_at)::bigint * 1000)::text
                    END AS last_commit_at_ms_text
                FROM project_tickets pt
                LEFT JOIN LATERAL (
                    SELECT
                        MAX(c.created_at) AS last_commit_at,
                        COUNT(*) AS commit_links
                    FROM commit_ticket_correlations ct
                    LEFT JOIN client_events c
                      ON c.commit_sha = ct.commit_sha
                     AND c.event_type = 'commit'
                     AND (c.org_id = pt.org_id OR c.org_id IS NULL)
                    WHERE ct.ticket_id = pt.ticket_id
                      AND (ct.org_id = pt.org_id OR ct.org_id IS NULL)
                ) corr ON TRUE
                WHERE pt.org_id = $1::uuid
                  AND COALESCE(pt.updated_at, pt.ingested_at) >= NOW() - make_interval(hours => $2::int)
                  AND (
                    lower(COALESCE(pt.status, '')) IN (
                        'in progress', 'in_progress', 'doing', 'open', 'todo', 'to do', 'in review'
                    )
                    OR lower(COALESCE(pt.status, '')) LIKE '%progress%'
                    OR lower(COALESCE(pt.status, '')) LIKE '%doing%'
                    OR lower(COALESCE(pt.status, '')) LIKE '%review%'
                  )
            ),
            candidates AS (
                SELECT ta.*
                FROM ticket_activity ta
                WHERE (
                        ta.last_commit_at IS NULL
                        OR ta.last_commit_at < NOW() - make_interval(days => $3::int)
                      )
                  AND NOT EXISTS (
                    SELECT 1
                    FROM noncompliance_signals ns
                    WHERE ns.org_id = ta.org_id
                      AND ns.signal_type = 'stale_in_progress'
                      AND ns.evidence->>'ticket_id' = ta.ticket_id
                      AND COALESCE(ns.evidence->>'last_commit_at', '') = ta.last_commit_at_ms_text
                  )
            )
            INSERT INTO noncompliance_signals (
                org_id,
                signal_type,
                confidence,
                actor_login,
                evidence,
                context
            )
            SELECT
                c.org_id,
                'stale_in_progress',
                'medium',
                COALESCE(NULLIF(c.assignee, ''), NULLIF(c.reporter, ''), 'system'),
                jsonb_build_object(
                    'ticket_id', c.ticket_id,
                    'ticket_status', c.status,
                    'ticket_updated_at', CASE
                        WHEN c.ticket_updated_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM c.ticket_updated_at)::bigint * 1000
                    END,
                    'last_commit_at', CASE
                        WHEN c.last_commit_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM c.last_commit_at)::bigint * 1000
                    END,
                    'correlated_commit_count', c.commit_links,
                    'reason', 'Ticket in progress without recent commit activity'
                ),
                jsonb_build_object(
                    'detection_window_hours', $2::int,
                    'stale_days', $3::int,
                    'source', 'v2_advanced'
                )
            FROM candidates c
            "#,
        )
        .bind(org_id)
        .bind(hours as i32)
        .bind(stale_days)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    async fn detect_v2_done_not_deployed_signals(
        &self,
        org_id: &str,
        done_window_hours: i64,
        pipeline_lookback_hours: i64,
    ) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            WITH done_tickets AS (
                SELECT
                    pt.org_id,
                    pt.ticket_id,
                    pt.status,
                    pt.title,
                    pt.priority,
                    pt.ticket_type,
                    pt.assignee,
                    pt.reporter,
                    COALESCE(pt.updated_at, pt.ingested_at) AS ticket_updated_at
                FROM project_tickets pt
                WHERE pt.org_id = $1::uuid
                  AND COALESCE(pt.updated_at, pt.ingested_at) >= NOW() - make_interval(hours => $2::int)
                  AND (
                    lower(COALESCE(pt.status, '')) IN ('done', 'closed', 'resolved')
                    OR lower(COALESCE(pt.status, '')) LIKE '%done%'
                    OR lower(COALESCE(pt.status, '')) LIKE '%closed%'
                    OR lower(COALESCE(pt.status, '')) LIKE '%resolved%'
                  )
            ),
            ticket_commits AS (
                SELECT
                    dt.*,
                    ct.commit_sha
                FROM done_tickets dt
                JOIN commit_ticket_correlations ct
                  ON ct.ticket_id = dt.ticket_id
                 AND (ct.org_id = dt.org_id OR ct.org_id IS NULL)
            ),
            ticket_pipeline_eval AS (
                SELECT
                    tc.org_id,
                    tc.ticket_id,
                    tc.status,
                    tc.title,
                    tc.priority,
                    tc.ticket_type,
                    tc.assignee,
                    tc.reporter,
                    tc.ticket_updated_at,
                    COUNT(DISTINCT tc.commit_sha)::bigint AS correlated_commit_count,
                    COALESCE(BOOL_OR(
                        lower(COALESCE(pe.status, '')) = 'success'
                        AND (
                            lower(COALESCE(pe.job_name, '')) LIKE '%deploy%'
                            OR lower(COALESCE(pe.job_name, '')) LIKE '%release%'
                            OR lower(COALESCE(pe.job_name, '')) LIKE '%prod%'
                            OR lower(COALESCE(pe.payload::text, '')) LIKE '%\"environment\":\"production\"%'
                            OR lower(COALESCE(pe.payload::text, '')) LIKE '%deploy%'
                        )
                    ), FALSE) AS has_successful_deploy,
                    MAX(pe.ingested_at) FILTER (WHERE lower(COALESCE(pe.status, '')) = 'success') AS last_success_pipeline_at,
                    MAX(pe.id::text) FILTER (WHERE lower(COALESCE(pe.status, '')) = 'success') AS last_success_pipeline_id
                FROM ticket_commits tc
                LEFT JOIN pipeline_events pe
                  ON pe.org_id = tc.org_id
                 AND pe.commit_sha IS NOT NULL
                 AND (
                    pe.commit_sha = tc.commit_sha
                    OR pe.commit_sha LIKE tc.commit_sha || '%'
                    OR tc.commit_sha LIKE pe.commit_sha || '%'
                 )
                 AND pe.ingested_at >= NOW() - make_interval(hours => $3::int)
                GROUP BY
                    tc.org_id,
                    tc.ticket_id,
                    tc.status,
                    tc.title,
                    tc.priority,
                    tc.ticket_type,
                    tc.assignee,
                    tc.reporter,
                    tc.ticket_updated_at
            ),
            candidates AS (
                SELECT tpe.*
                FROM ticket_pipeline_eval tpe
                WHERE tpe.correlated_commit_count > 0
                  AND tpe.has_successful_deploy = FALSE
                  AND NOT EXISTS (
                    SELECT 1
                    FROM noncompliance_signals ns
                    WHERE ns.org_id = tpe.org_id
                      AND ns.signal_type = 'done_not_deployed'
                      AND ns.evidence->>'ticket_id' = tpe.ticket_id
                  )
            )
            INSERT INTO noncompliance_signals (
                org_id,
                signal_type,
                confidence,
                actor_login,
                evidence,
                context
            )
            SELECT
                c.org_id,
                'done_not_deployed',
                'high',
                COALESCE(NULLIF(c.assignee, ''), NULLIF(c.reporter, ''), 'system'),
                jsonb_build_object(
                    'ticket_id', c.ticket_id,
                    'ticket_status', c.status,
                    'ticket_updated_at', CASE
                        WHEN c.ticket_updated_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM c.ticket_updated_at)::bigint * 1000
                    END,
                    'correlated_commit_count', c.correlated_commit_count,
                    'last_success_pipeline_at', CASE
                        WHEN c.last_success_pipeline_at IS NULL THEN NULL
                        ELSE EXTRACT(EPOCH FROM c.last_success_pipeline_at)::bigint * 1000
                    END,
                    'last_success_pipeline_id', c.last_success_pipeline_id,
                    'reason', 'Done ticket has correlated commits but no successful deployment-like pipeline'
                ),
                jsonb_build_object(
                    'done_window_hours', $2::int,
                    'pipeline_lookback_hours', $3::int,
                    'source', 'v2_advanced'
                )
            FROM candidates c
            "#,
        )
        .bind(org_id)
        .bind(done_window_hours as i32)
        .bind(pipeline_lookback_hours as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    pub async fn detect_noncompliance_signals(&self, org_id: &str) -> Result<i64, DbError> {
        // Legacy SQL detector can be unavailable/misaligned when a deployment
        // has partial migrations. Keep detection resilient and continue with
        // V2 server-side rules instead of failing the endpoint/job.
        let mut total_created: i64 = match sqlx::query(
            "SELECT detect_noncompliance_signals($1::uuid)::bigint as count",
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        {
            Ok(row) => row.get("count"),
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "Legacy SQL detect_noncompliance_signals failed; continuing with V2 fallback detection"
                );
                0
            }
        };
        let commit_window_hours = 24 * 7;
        let ticket_window_hours = 24 * 30;
        let pipeline_streak_window_hours = 24 * 14;
        let stale_in_progress_window_hours = 24 * 30;
        let done_ticket_window_hours = 24 * 45;
        let pipeline_lookback_hours = 24 * 45;

        match self
            .detect_v2_commit_no_ticket_signals(org_id, commit_window_hours)
            .await
        {
            Ok(count) => {
                total_created += count;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "V2 commit_no_ticket detection skipped due to database error"
                );
            }
        }

        match self
            .detect_v2_ticket_no_coverage_signals(org_id, ticket_window_hours)
            .await
        {
            Ok(count) => {
                total_created += count;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "V2 ticket_no_coverage detection skipped due to database error"
                );
            }
        }

        match self
            .detect_v2_pipeline_failure_streak_signals(org_id, pipeline_streak_window_hours, 3)
            .await
        {
            Ok(count) => {
                total_created += count;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "V2 pipeline_failure_streak detection skipped due to database error"
                );
            }
        }

        match self
            .detect_v2_stale_in_progress_signals(org_id, stale_in_progress_window_hours, 3)
            .await
        {
            Ok(count) => {
                total_created += count;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "V2 stale_in_progress detection skipped due to database error"
                );
            }
        }

        match self
            .detect_v2_done_not_deployed_signals(
                org_id,
                done_ticket_window_hours,
                pipeline_lookback_hours,
            )
            .await
        {
            Ok(count) => {
                total_created += count;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "V2 done_not_deployed detection skipped due to database error"
                );
            }
        }

        Ok(total_created)
    }

    // ========================================================================
    // COMPLIANCE DASHBOARD
    // ========================================================================

    async fn get_compliance_timeline_monthly(
        &self,
        org_id: &str,
        months: i64,
    ) -> Result<Vec<ComplianceTimelinePoint>, DbError> {
        let safe_months = months.clamp(1, 24) as i32;
        let rows = sqlx::query(
            r#"
            WITH bounds AS (
              SELECT
                date_trunc('month', (NOW() AT TIME ZONE 'UTC'))::date AS end_month,
                (
                  date_trunc('month', (NOW() AT TIME ZONE 'UTC'))::date
                  - (($2::int - 1) * INTERVAL '1 month')
                )::date AS start_month
            ),
            months AS (
              SELECT generate_series(
                (SELECT start_month FROM bounds),
                (SELECT end_month FROM bounds),
                INTERVAL '1 month'
              )::date AS month_start
            ),
            signals AS (
              SELECT
                date_trunc('month', ns.created_at AT TIME ZONE 'UTC')::date AS month_start,
                COUNT(*)::bigint AS signals_detected
              FROM noncompliance_signals ns
              WHERE ns.org_id = $1::uuid
                AND ns.created_at >= (SELECT start_month::timestamp FROM bounds)
              GROUP BY 1
            ),
            violations AS (
              SELECT
                date_trunc('month', v.created_at AT TIME ZONE 'UTC')::date AS month_start,
                COUNT(*)::bigint AS violations_confirmed
              FROM violations v
              WHERE v.org_id = $1::uuid
                AND v.created_at >= (SELECT start_month::timestamp FROM bounds)
              GROUP BY 1
            ),
            commit_coverage AS (
              SELECT
                date_trunc('month', ce.created_at AT TIME ZONE 'UTC')::date AS month_start,
                COUNT(DISTINCT ce.commit_sha)::bigint AS commits_total,
                COUNT(DISTINCT CASE WHEN ctc.commit_sha IS NOT NULL THEN ce.commit_sha END)::bigint AS commits_with_ticket
              FROM client_events ce
              LEFT JOIN (
                SELECT DISTINCT org_id, commit_sha
                FROM commit_ticket_correlations
                WHERE org_id = $1::uuid
              ) ctc
                ON ctc.org_id = ce.org_id
               AND ctc.commit_sha = ce.commit_sha
              WHERE ce.org_id = $1::uuid
                AND ce.event_type = 'commit'
                AND ce.commit_sha IS NOT NULL
                AND ce.created_at >= (SELECT start_month::timestamp FROM bounds)
              GROUP BY 1
            ),
            pipeline AS (
              SELECT
                date_trunc('month', pe.ingested_at AT TIME ZONE 'UTC')::date AS month_start,
                COUNT(*)::bigint AS pipeline_runs_total,
                COUNT(*) FILTER (WHERE pe.status = 'success')::bigint AS pipeline_runs_success
              FROM pipeline_events pe
              WHERE pe.org_id = $1::uuid
                AND pe.ingested_at >= (SELECT start_month::timestamp FROM bounds)
              GROUP BY 1
            )
            SELECT
              to_char(m.month_start, 'YYYY-MM') AS month,
              COALESCE(s.signals_detected, 0)::bigint AS signals_detected,
              COALESCE(v.violations_confirmed, 0)::bigint AS violations_confirmed,
              COALESCE(c.commits_total, 0)::bigint AS commits_total,
              COALESCE(c.commits_with_ticket, 0)::bigint AS commits_with_ticket,
              CASE
                WHEN COALESCE(c.commits_total, 0) > 0 THEN
                  ROUND((COALESCE(c.commits_with_ticket, 0)::numeric * 100.0) / NULLIF(c.commits_total, 0), 1)::double precision
                ELSE 100.0
              END AS ticket_coverage_pct,
              COALESCE(p.pipeline_runs_total, 0)::bigint AS pipeline_runs_total,
              CASE
                WHEN COALESCE(p.pipeline_runs_total, 0) > 0 THEN
                  ROUND((COALESCE(p.pipeline_runs_success, 0)::numeric * 100.0) / NULLIF(p.pipeline_runs_total, 0), 1)::double precision
                ELSE 100.0
              END AS pipeline_success_pct
            FROM months m
            LEFT JOIN signals s ON s.month_start = m.month_start
            LEFT JOIN violations v ON v.month_start = m.month_start
            LEFT JOIN commit_coverage c ON c.month_start = m.month_start
            LEFT JOIN pipeline p ON p.month_start = m.month_start
            ORDER BY m.month_start ASC
            "#,
        )
        .bind(org_id)
        .bind(safe_months)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|row| ComplianceTimelinePoint {
                month: row.get("month"),
                signals_detected: row.get("signals_detected"),
                violations_confirmed: row.get("violations_confirmed"),
                commits_total: row.get("commits_total"),
                commits_with_ticket: row.get("commits_with_ticket"),
                ticket_coverage_pct: row.get("ticket_coverage_pct"),
                pipeline_runs_total: row.get("pipeline_runs_total"),
                pipeline_success_pct: row.get("pipeline_success_pct"),
            })
            .collect())
    }

    pub async fn get_compliance_dashboard(
        &self,
        org_id: &str,
    ) -> Result<ComplianceDashboard, DbError> {
        let row = sqlx::query("SELECT get_compliance_dashboard($1::uuid) as dashboard")
            .bind(org_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let mut dashboard_value: serde_json::Value = row
            .try_get::<sqlx::types::Json<serde_json::Value>, _>("dashboard")
            .map(|json| json.0)
            .or_else(|_| row.try_get::<serde_json::Value, _>("dashboard"))
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        if let Some(obj) = dashboard_value.as_object_mut() {
            for key in ["signals", "correlation", "policy", "exports"] {
                let is_null = obj.get(key).map(|v| v.is_null()).unwrap_or(false);
                if is_null {
                    obj.remove(key);
                }
            }
            let timeline_is_null = obj.get("timeline").map(|v| v.is_null()).unwrap_or(false);
            if timeline_is_null {
                obj.insert("timeline".to_string(), serde_json::json!([]));
            }
            if let Some(signals_obj) = obj.get_mut("signals").and_then(|v| v.as_object_mut()) {
                let by_type_is_null = signals_obj
                    .get("by_type")
                    .map(|v| v.is_null())
                    .unwrap_or(false);
                if by_type_is_null {
                    signals_obj.insert("by_type".to_string(), serde_json::json!({}));
                }
            }
        }

        let mut resolved = match serde_json::from_value::<ComplianceDashboard>(dashboard_value) {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "Failed to deserialize compliance dashboard payload; using defaults"
                );
                ComplianceDashboard::default()
            }
        };
        match self.get_compliance_timeline_monthly(org_id, 6).await {
            Ok(timeline) => {
                resolved.timeline = timeline;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %org_id,
                    error = %e,
                    "Monthly compliance timeline skipped due to database error"
                );
            }
        }

        Ok(resolved)
    }

    // ========================================================================
    // POLICY HISTORY
    // ========================================================================

    pub async fn get_policy_history(&self, repo_id: &str) -> Result<Vec<PolicyHistory>, DbError> {
        let rows = sqlx::query("SELECT * FROM get_policy_history($1::uuid)")
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

    pub async fn create_policy_change_request(
        &self,
        input: CreatePolicyChangeRequestInput<'_>,
    ) -> Result<(), DbError> {
        let requested_config_json = serde_json::to_value(input.requested_config)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO policy_change_requests (
                id, org_id, repo_id, repo_name, requested_by,
                requested_config, requested_checksum, reason, created_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3::uuid, $4, $5,
                $6::jsonb, $7, $8, to_timestamp($9::bigint / 1000.0)
            )
            "#,
        )
        .bind(input.request_id)
        .bind(input.org_id)
        .bind(input.repo_id)
        .bind(input.repo_name)
        .bind(input.requested_by)
        .bind(&requested_config_json)
        .bind(input.requested_checksum)
        .bind(input.reason)
        .bind(input.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn list_policy_change_requests(
        &self,
        input: ListPolicyChangeRequestsInput<'_>,
    ) -> Result<(Vec<PolicyChangeRequestRecord>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                r.id::text AS id,
                r.org_id::text AS org_id,
                r.repo_id::text AS repo_id,
                r.repo_name AS repo_name,
                r.requested_by AS requested_by,
                r.requested_checksum AS requested_checksum,
                CASE
                  WHEN $7::boolean THEN r.requested_config
                  ELSE '{}'::jsonb
                END AS requested_config,
                r.reason AS reason,
                COALESCE(d.decision, 'pending') AS status,
                d.decided_by AS decided_by,
                d.note AS decision_note,
                EXTRACT(EPOCH FROM r.created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM d.created_at)::bigint * 1000 AS decided_at_ms
            FROM policy_change_requests r
            LEFT JOIN policy_change_request_decisions d
              ON d.request_id = r.id
            WHERE ($1::uuid IS NULL OR r.org_id = $1::uuid)
              AND ($2::text IS NULL OR r.repo_name = $2)
              AND ($3::text IS NULL OR r.requested_by = $3)
              AND ($4::text IS NULL OR COALESCE(d.decision, 'pending') = $4)
            ORDER BY r.created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(input.org_id)
        .bind(input.repo_name)
        .bind(input.requested_by)
        .bind(input.status)
        .bind(input.limit)
        .bind(input.offset)
        .bind(input.include_config)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM policy_change_requests r
            LEFT JOIN policy_change_request_decisions d
              ON d.request_id = r.id
            WHERE ($1::uuid IS NULL OR r.org_id = $1::uuid)
              AND ($2::text IS NULL OR r.repo_name = $2)
              AND ($3::text IS NULL OR r.requested_by = $3)
              AND ($4::text IS NULL OR COALESCE(d.decision, 'pending') = $4)
            "#,
        )
        .bind(input.org_id)
        .bind(input.repo_name)
        .bind(input.requested_by)
        .bind(input.status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let records = rows
            .iter()
            .map(|row| {
                let config: serde_json::Value = row.get("requested_config");
                PolicyChangeRequestRecord {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    repo_id: row.get("repo_id"),
                    repo_name: row.get("repo_name"),
                    requested_by: row.get("requested_by"),
                    requested_checksum: row.get("requested_checksum"),
                    requested_config: serde_json::from_value(config).unwrap_or_default(),
                    reason: row.get("reason"),
                    status: row.get("status"),
                    decided_by: row.get("decided_by"),
                    decision_note: row.get("decision_note"),
                    created_at: row.get("created_at_ms"),
                    decided_at: row.get("decided_at_ms"),
                }
            })
            .collect();

        Ok((records, count))
    }

    pub async fn get_policy_change_request_by_id(
        &self,
        request_id: &str,
        org_id: Option<&str>,
    ) -> Result<Option<PolicyChangeRequestRecord>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                r.id::text AS id,
                r.org_id::text AS org_id,
                r.repo_id::text AS repo_id,
                r.repo_name AS repo_name,
                r.requested_by AS requested_by,
                r.requested_checksum AS requested_checksum,
                r.requested_config AS requested_config,
                r.reason AS reason,
                COALESCE(d.decision, 'pending') AS status,
                d.decided_by AS decided_by,
                d.note AS decision_note,
                EXTRACT(EPOCH FROM r.created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM d.created_at)::bigint * 1000 AS decided_at_ms
            FROM policy_change_requests r
            LEFT JOIN policy_change_request_decisions d
              ON d.request_id = r.id
            WHERE r.id = $1::uuid
              AND ($2::uuid IS NULL OR r.org_id = $2::uuid)
            "#,
        )
        .bind(request_id)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let config: serde_json::Value = row.get("requested_config");
        Ok(Some(PolicyChangeRequestRecord {
            id: row.get("id"),
            org_id: row.get("org_id"),
            repo_id: row.get("repo_id"),
            repo_name: row.get("repo_name"),
            requested_by: row.get("requested_by"),
            requested_checksum: row.get("requested_checksum"),
            requested_config: serde_json::from_value(config)
                .map_err(|e| DbError::SerializationError(e.to_string()))?,
            reason: row.get("reason"),
            status: row.get("status"),
            decided_by: row.get("decided_by"),
            decision_note: row.get("decision_note"),
            created_at: row.get("created_at_ms"),
            decided_at: row.get("decided_at_ms"),
        }))
    }

    pub async fn approve_policy_change_request(
        &self,
        request_id: &str,
        org_id: Option<&str>,
        decided_by: &str,
        note: Option<&str>,
        decided_at_ms: i64,
    ) -> Result<PolicyChangeRequestRecord, DbError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let request_row = sqlx::query(
            r#"
            SELECT
                id::text AS id,
                org_id::text AS org_id,
                repo_id::text AS repo_id,
                repo_name AS repo_name,
                requested_by AS requested_by,
                requested_checksum AS requested_checksum,
                requested_config AS requested_config,
                reason AS reason,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM policy_change_requests
            WHERE id = $1::uuid
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            FOR UPDATE
            "#,
        )
        .bind(request_id)
        .bind(org_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let Some(request_row) = request_row else {
            return Err(DbError::NotFound("policy_change_request".to_string()));
        };

        let existing_decision: Option<String> = sqlx::query_scalar(
            r#"
            SELECT decision
            FROM policy_change_request_decisions
            WHERE request_id = $1::uuid
            LIMIT 1
            "#,
        )
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        if existing_decision.is_some() {
            return Err(DbError::Duplicate(
                "policy_change_request already decided".to_string(),
            ));
        }

        let requested_config_json: serde_json::Value = request_row.get("requested_config");
        let requested_checksum: String = request_row.get("requested_checksum");
        let repo_id: String = request_row.get("repo_id");

        sqlx::query(
            r#"
            INSERT INTO policy_change_request_decisions (
                id, request_id, org_id, decision, decided_by, note, created_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3::uuid, 'approved', $4, $5, to_timestamp($6::bigint / 1000.0)
            )
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(request_id)
        .bind(request_row.get::<Option<String>, _>("org_id"))
        .bind(decided_by)
        .bind(note)
        .bind(decided_at_ms)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO policies (repo_id, config, checksum, override_actor, updated_at)
            VALUES ($1::uuid, $2::jsonb, $3, $4, NOW())
            ON CONFLICT (repo_id) DO UPDATE SET
                config = $2,
                checksum = $3,
                override_actor = $4,
                updated_at = NOW()
            "#,
        )
        .bind(&repo_id)
        .bind(&requested_config_json)
        .bind(&requested_checksum)
        .bind(decided_by)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let requested_config: GitGovConfig = serde_json::from_value(requested_config_json)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;
        Ok(PolicyChangeRequestRecord {
            id: request_row.get("id"),
            org_id: request_row.get("org_id"),
            repo_id,
            repo_name: request_row.get("repo_name"),
            requested_by: request_row.get("requested_by"),
            requested_checksum,
            requested_config,
            reason: request_row.get("reason"),
            status: "approved".to_string(),
            decided_by: Some(decided_by.to_string()),
            decision_note: note.map(str::to_string),
            created_at: request_row.get("created_at_ms"),
            decided_at: Some(decided_at_ms),
        })
    }

    pub async fn reject_policy_change_request(
        &self,
        request_id: &str,
        org_id: Option<&str>,
        decided_by: &str,
        note: Option<&str>,
        decided_at_ms: i64,
    ) -> Result<PolicyChangeRequestRecord, DbError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let request_row = sqlx::query(
            r#"
            SELECT
                id::text AS id,
                org_id::text AS org_id,
                repo_id::text AS repo_id,
                repo_name AS repo_name,
                requested_by AS requested_by,
                requested_checksum AS requested_checksum,
                requested_config AS requested_config,
                reason AS reason,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM policy_change_requests
            WHERE id = $1::uuid
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            FOR UPDATE
            "#,
        )
        .bind(request_id)
        .bind(org_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let Some(request_row) = request_row else {
            return Err(DbError::NotFound("policy_change_request".to_string()));
        };

        let existing_decision: Option<String> = sqlx::query_scalar(
            r#"
            SELECT decision
            FROM policy_change_request_decisions
            WHERE request_id = $1::uuid
            LIMIT 1
            "#,
        )
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        if existing_decision.is_some() {
            return Err(DbError::Duplicate(
                "policy_change_request already decided".to_string(),
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO policy_change_request_decisions (
                id, request_id, org_id, decision, decided_by, note, created_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3::uuid, 'rejected', $4, $5, to_timestamp($6::bigint / 1000.0)
            )
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(request_id)
        .bind(request_row.get::<Option<String>, _>("org_id"))
        .bind(decided_by)
        .bind(note)
        .bind(decided_at_ms)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let requested_config_json: serde_json::Value = request_row.get("requested_config");
        let requested_config: GitGovConfig = serde_json::from_value(requested_config_json)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;
        Ok(PolicyChangeRequestRecord {
            id: request_row.get("id"),
            org_id: request_row.get("org_id"),
            repo_id: request_row.get("repo_id"),
            repo_name: request_row.get("repo_name"),
            requested_by: request_row.get("requested_by"),
            requested_checksum: request_row.get("requested_checksum"),
            requested_config,
            reason: request_row.get("reason"),
            status: "rejected".to_string(),
            decided_by: Some(decided_by.to_string()),
            decision_note: note.map(str::to_string),
            created_at: request_row.get("created_at_ms"),
            decided_at: Some(decided_at_ms),
        })
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
        .bind(export.date_range_start.map(chrono::DateTime::from_timestamp_millis))
        .bind(export.date_range_end.map(chrono::DateTime::from_timestamp_millis))
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
            Ok(res) if res.rows_affected() == 0 => Err(DbError::Duplicate(format!(
                "delivery_id: {}",
                event.delivery_id
            ))),
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate") => Err(DbError::Duplicate(format!(
                "delivery_id: {}",
                event.delivery_id
            ))),
            Err(e) => Err(DbError::DatabaseError(e.to_string())),
        }
    }

    pub async fn insert_governance_events_batch(
        &self,
        events: &[GovernanceEvent],
    ) -> Result<(i32, Vec<String>), DbError> {
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
        let count = self.detect_noncompliance_signals(org_id).await?;

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

    // ========================================================================
    // GDPR — T2 (art. 17 erasure, art. 20 export, TTL cleanup)
    // ========================================================================

    /// Register GDPR erasure request for a user.
    /// Audit tables are append-only, so this records intent and returns scoped counts only.
    /// Returns (client_events_matched, github_events_matched).
    pub async fn erase_user_data(
        &self,
        user_login: &str,
        org_id: Option<&str>,
    ) -> Result<(i64, i64), DbError> {
        let client_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::bigint
            FROM client_events
            WHERE user_login = $1
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            "#,
        )
        .bind(user_login)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let github_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::bigint
            FROM github_events
            WHERE actor_login = $1
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            "#,
        )
        .bind(user_login)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        // Only record erasure intent if there is data in the visible scope.
        if client_count > 0 || github_count > 0 {
            sqlx::query(
                r#"
                INSERT INTO user_pseudonyms (user_login, erased_at)
                VALUES ($1, NOW())
                ON CONFLICT (user_login) DO UPDATE SET erased_at = NOW()
                "#,
            )
            .bind(user_login)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;
        }

        Ok((client_count, github_count))
    }

    /// Export all events for a user (GDPR art. 20 data portability).
    pub async fn export_user_data(
        &self,
        user_login: &str,
        org_id: Option<&str>,
    ) -> Result<Vec<CombinedEvent>, DbError> {
        let filter = EventFilter {
            user_login: Some(user_login.to_string()),
            org_id: org_id.map(str::to_string),
            limit: 50_000,
            ..Default::default()
        };
        self.get_combined_events(&filter).await
    }

    /// Delete client session rows older than `retention_days` days.
    /// Audit events remain append-only by design.
    /// Returns number of rows deleted.
    pub async fn delete_old_events(&self, retention_days: i64) -> Result<i64, DbError> {
        let result = sqlx::query(
            r#"
            DELETE FROM client_sessions
            WHERE last_seen_at < NOW() - ($1::bigint * INTERVAL '1 day')
            "#,
        )
        .bind(retention_days)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    // ========================================================================
    // CLIENT SESSIONS — T3.A (heartbeat / last_seen)
    // ========================================================================

    /// Upsert client session — called on every inbound event + heartbeat.
    pub async fn upsert_client_session(
        &self,
        client_id: &str,
        org_id: Option<&str>,
        device_metadata: &serde_json::Value,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO client_sessions (client_id, org_id, last_seen_at, device_metadata)
            VALUES ($1, $2::uuid, NOW(), $3::jsonb)
            ON CONFLICT (client_id) DO UPDATE SET
                last_seen_at    = NOW(),
                device_metadata = EXCLUDED.device_metadata,
                org_id          = COALESCE(EXCLUDED.org_id, client_sessions.org_id)
            "#,
        )
        .bind(client_id)
        .bind(org_id)
        .bind(device_metadata.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// List client sessions (for GET /clients), scoped by org.
    pub async fn get_client_sessions(
        &self,
        org_id: Option<&str>,
    ) -> Result<Vec<ClientSession>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                client_id,
                org_id::text,
                EXTRACT(EPOCH FROM last_seen_at)::bigint * 1000 AS last_seen_ms,
                COALESCE(device_metadata, '{}')::text            AS device_metadata,
                EXTRACT(EPOCH FROM created_at)::bigint  * 1000  AS created_at_ms
            FROM client_sessions
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
            ORDER BY last_seen_at DESC
            LIMIT 500
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let sessions = rows
            .iter()
            .map(|r| {
                let last_seen_ms: i64 = r.get("last_seen_ms");
                ClientSession {
                    client_id: r.get("client_id"),
                    org_id: r.get("org_id"),
                    last_seen_at: last_seen_ms,
                    device_metadata: serde_json::from_str(r.get::<&str, _>("device_metadata"))
                        .unwrap_or_default(),
                    created_at: r.get("created_at_ms"),
                    is_active: last_seen_ms > (now_ms - 86_400_000), // active = seen in last 24h
                }
            })
            .collect();

        Ok(sessions)
    }

    // ========================================================================
    // IDENTITY ALIASES — T3.B
    // ========================================================================

    /// Map alias_login → canonical_login (idempotent on alias conflict).
    /// Returns true if newly created, false if alias already mapped.
    pub async fn create_identity_alias(
        &self,
        canonical: &str,
        alias: &str,
        org_id: Option<&str>,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            r#"
            INSERT INTO identity_aliases (canonical_login, alias_login, org_id)
            VALUES ($1, $2, $3::uuid)
            ON CONFLICT (alias_login) DO NOTHING
            "#,
        )
        .bind(canonical)
        .bind(alias)
        .bind(org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// List identity aliases, optionally scoped by org.
    pub async fn list_identity_aliases(
        &self,
        org_id: Option<&str>,
    ) -> Result<Vec<IdentityAlias>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT canonical_login, alias_login, org_id::text,
                   EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM identity_aliases
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
            ORDER BY canonical_login, alias_login
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| IdentityAlias {
                canonical_login: r.get("canonical_login"),
                alias_login: r.get("alias_login"),
                org_id: r.get("org_id"),
                created_at: r.get("created_at_ms"),
            })
            .collect())
    }

    // ========================================================================
    // ORG USERS — V1.4-A
    // ========================================================================

    fn row_to_org_user(row: &sqlx::postgres::PgRow) -> OrgUser {
        OrgUser {
            id: row.get("id"),
            org_id: row.get("org_id"),
            login: row.get("login"),
            display_name: row.get("display_name"),
            email: row.get("email"),
            role: row.get("role"),
            status: row.get("status"),
            created_by: row.get("created_by"),
            updated_by: row.get("updated_by"),
            created_at: row.get("created_at_ms"),
            updated_at: row.get("updated_at_ms"),
        }
    }

    pub async fn upsert_org_user(
        &self,
        input: &UpsertOrgUserInput<'_>,
    ) -> Result<(OrgUser, bool), DbError> {
        let existing_id = sqlx::query(
            r#"
            SELECT id::text AS id
            FROM org_users
            WHERE org_id = $1::uuid
              AND login = $2
            "#,
        )
        .bind(input.org_id)
        .bind(input.login)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?
        .map(|r| r.get::<String, _>("id"));

        let created = existing_id.is_none();
        let row = if let Some(id) = existing_id {
            sqlx::query(
                r#"
                UPDATE org_users
                SET
                    display_name = COALESCE($2, display_name),
                    email        = COALESCE($3, email),
                    role         = $4,
                    status       = $5,
                    updated_by   = $6,
                    updated_at   = NOW()
                WHERE id = $1::uuid
                RETURNING
                    id::text,
                    org_id::text,
                    login,
                    display_name,
                    email,
                    role,
                    status,
                    created_by,
                    updated_by,
                    EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                    EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
                "#,
            )
            .bind(&id)
            .bind(input.display_name)
            .bind(input.email)
            .bind(input.role)
            .bind(input.status)
            .bind(input.actor)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?
        } else {
            sqlx::query(
                r#"
                INSERT INTO org_users (
                    org_id, login, display_name, email, role, status, created_by, updated_by
                )
                VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $7)
                RETURNING
                    id::text,
                    org_id::text,
                    login,
                    display_name,
                    email,
                    role,
                    status,
                    created_by,
                    updated_by,
                    EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                    EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
                "#,
            )
            .bind(input.org_id)
            .bind(input.login)
            .bind(input.display_name)
            .bind(input.email)
            .bind(input.role)
            .bind(input.status)
            .bind(input.actor)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?
        };

        Ok((Self::row_to_org_user(&row), created))
    }

    pub async fn list_org_users(
        &self,
        org_id: &str,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<OrgUser>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                login,
                display_name,
                email,
                role,
                status,
                created_by,
                updated_by,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            FROM org_users
            WHERE org_id = $1::uuid
              AND ($2::text IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(org_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM org_users
            WHERE org_id = $1::uuid
              AND ($2::text IS NULL OR status = $2)
            "#,
        )
        .bind(org_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");
        let entries = rows.iter().map(Self::row_to_org_user).collect();
        Ok((entries, total))
    }

    pub async fn get_org_user_by_id(
        &self,
        org_user_id: &str,
        scope_org_id: Option<&str>,
    ) -> Result<Option<OrgUser>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                login,
                display_name,
                email,
                role,
                status,
                created_by,
                updated_by,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            FROM org_users
            WHERE id = $1::uuid
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            "#,
        )
        .bind(org_user_id)
        .bind(scope_org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| Self::row_to_org_user(&r)))
    }

    pub async fn update_org_user_status(
        &self,
        org_user_id: &str,
        scope_org_id: Option<&str>,
        status: &str,
        actor: &str,
    ) -> Result<Option<OrgUser>, DbError> {
        let row = sqlx::query(
            r#"
            UPDATE org_users
            SET
                status     = $3,
                updated_by = $4,
                updated_at = NOW()
            WHERE id = $1::uuid
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            RETURNING
                id::text,
                org_id::text,
                login,
                display_name,
                email,
                role,
                status,
                created_by,
                updated_by,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            "#,
        )
        .bind(org_user_id)
        .bind(scope_org_id)
        .bind(status)
        .bind(actor)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| Self::row_to_org_user(&r)))
    }

    pub async fn get_team_overview(
        &self,
        org_id: &str,
        status: Option<&str>,
        days: i64,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<TeamDeveloperOverview>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            WITH filtered_users AS (
                SELECT
                    ou.id,
                    ou.login,
                    ou.display_name,
                    ou.email,
                    ou.role,
                    ou.status,
                    ou.created_at
                FROM org_users ou
                WHERE ou.org_id = $1::uuid
                  AND ($2::text IS NULL OR ou.status = $2)
                ORDER BY ou.created_at DESC
                LIMIT $3 OFFSET $4
            ),
            window_events AS (
                SELECT
                    COALESCE(ica.canonical_login, c.user_login) AS user_login,
                    COALESCE(r.full_name, c.metadata->>'repo_name') AS repo_name,
                    c.event_type,
                    c.status,
                    c.created_at
                FROM client_events c
                LEFT JOIN repos r ON r.id = c.repo_id
                LEFT JOIN identity_aliases ica
                    ON ica.alias_login = c.user_login
                   AND ica.org_id = $1::uuid
                WHERE c.org_id = $1::uuid
                  AND c.created_at >= NOW() - (($5::int || ' days')::interval)
            ),
            user_metrics AS (
                SELECT
                    we.user_login,
                    MAX(we.created_at) AS last_seen,
                    COUNT(*)::bigint AS total_events,
                    COUNT(*) FILTER (WHERE we.event_type = 'commit')::bigint AS commits,
                    COUNT(*) FILTER (WHERE we.event_type IN ('attempt_push', 'successful_push', 'push'))::bigint AS pushes,
                    COUNT(*) FILTER (WHERE we.event_type = 'blocked_push' OR we.status = 'blocked')::bigint AS blocked_pushes
                FROM window_events we
                GROUP BY we.user_login
            ),
            user_repo_metrics AS (
                SELECT
                    we.user_login,
                    we.repo_name,
                    COUNT(*)::bigint AS events,
                    COUNT(*) FILTER (WHERE we.event_type = 'commit')::bigint AS commits,
                    COUNT(*) FILTER (WHERE we.event_type IN ('attempt_push', 'successful_push', 'push'))::bigint AS pushes,
                    COUNT(*) FILTER (WHERE we.event_type = 'blocked_push' OR we.status = 'blocked')::bigint AS blocked_pushes,
                    EXTRACT(EPOCH FROM MAX(we.created_at))::bigint * 1000 AS last_seen_ms
                FROM window_events we
                WHERE we.repo_name IS NOT NULL AND we.repo_name <> ''
                GROUP BY we.user_login, we.repo_name
            )
            SELECT
                fu.login,
                fu.display_name,
                fu.email,
                fu.role,
                fu.status,
                EXTRACT(EPOCH FROM um.last_seen)::bigint * 1000 AS last_seen_ms,
                COALESCE(um.total_events, 0)::bigint AS total_events,
                COALESCE(um.commits, 0)::bigint AS commits,
                COALESCE(um.pushes, 0)::bigint AS pushes,
                COALESCE(um.blocked_pushes, 0)::bigint AS blocked_pushes,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM user_repo_metrics urm_cnt
                    WHERE urm_cnt.user_login = fu.login
                ), 0)::bigint AS repos_active_count,
                COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'repo_name', urm.repo_name,
                            'events', urm.events,
                            'commits', urm.commits,
                            'pushes', urm.pushes,
                            'blocked_pushes', urm.blocked_pushes,
                            'last_seen', urm.last_seen_ms
                        )
                        ORDER BY urm.events DESC, urm.repo_name ASC
                    )
                    FROM user_repo_metrics urm
                    WHERE urm.user_login = fu.login
                ), '[]'::jsonb) AS repos
            FROM filtered_users fu
            LEFT JOIN user_metrics um ON um.user_login = fu.login
            ORDER BY fu.created_at DESC
            "#,
        )
        .bind(org_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .bind(days)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM org_users
            WHERE org_id = $1::uuid
              AND ($2::text IS NULL OR status = $2)
            "#,
        )
        .bind(org_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");
        let entries = rows
            .iter()
            .map(|row| {
                let repos_json: serde_json::Value = row.get("repos");
                let repos: Vec<TeamRepoSummary> =
                    serde_json::from_value(repos_json).unwrap_or_default();
                TeamDeveloperOverview {
                    login: row.get("login"),
                    display_name: row.get("display_name"),
                    email: row.get("email"),
                    role: row.get("role"),
                    status: row.get("status"),
                    last_seen: row.get("last_seen_ms"),
                    total_events: row.get("total_events"),
                    commits: row.get("commits"),
                    pushes: row.get("pushes"),
                    blocked_pushes: row.get("blocked_pushes"),
                    repos_active_count: row.get("repos_active_count"),
                    repos,
                }
            })
            .collect();

        Ok((entries, total))
    }

    pub async fn get_team_repos(
        &self,
        org_id: &str,
        days: i64,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<TeamRepoOverview>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            WITH window_events AS (
                SELECT
                    COALESCE(ica.canonical_login, c.user_login) AS user_login,
                    COALESCE(r.full_name, c.metadata->>'repo_name') AS repo_name,
                    c.event_type,
                    c.status,
                    c.created_at
                FROM client_events c
                LEFT JOIN repos r ON r.id = c.repo_id
                LEFT JOIN identity_aliases ica
                    ON ica.alias_login = c.user_login
                   AND ica.org_id = $1::uuid
                WHERE c.org_id = $1::uuid
                  AND c.created_at >= NOW() - (($2::int || ' days')::interval)
            )
            SELECT
                we.repo_name,
                COUNT(DISTINCT we.user_login)::bigint AS developers_active,
                COUNT(*)::bigint AS total_events,
                COUNT(*) FILTER (WHERE we.event_type = 'commit')::bigint AS commits,
                COUNT(*) FILTER (WHERE we.event_type IN ('attempt_push', 'successful_push', 'push'))::bigint AS pushes,
                COUNT(*) FILTER (WHERE we.event_type = 'blocked_push' OR we.status = 'blocked')::bigint AS blocked_pushes,
                EXTRACT(EPOCH FROM MAX(we.created_at))::bigint * 1000 AS last_seen_ms
            FROM window_events we
            WHERE we.repo_name IS NOT NULL AND we.repo_name <> ''
            GROUP BY we.repo_name
            ORDER BY total_events DESC, we.repo_name ASC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(org_id)
        .bind(days)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count_row = sqlx::query(
            r#"
            WITH window_events AS (
                SELECT
                    COALESCE(r.full_name, c.metadata->>'repo_name') AS repo_name
                FROM client_events c
                LEFT JOIN repos r ON r.id = c.repo_id
                WHERE c.org_id = $1::uuid
                  AND c.created_at >= NOW() - (($2::int || ' days')::interval)
            )
            SELECT COUNT(DISTINCT repo_name) AS total
            FROM window_events
            WHERE repo_name IS NOT NULL AND repo_name <> ''
            "#,
        )
        .bind(org_id)
        .bind(days)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");
        let entries = rows
            .iter()
            .map(|row| TeamRepoOverview {
                repo_name: row.get("repo_name"),
                developers_active: row.get("developers_active"),
                total_events: row.get("total_events"),
                commits: row.get("commits"),
                pushes: row.get("pushes"),
                blocked_pushes: row.get("blocked_pushes"),
                last_seen: row.get("last_seen_ms"),
            })
            .collect();

        Ok((entries, total))
    }

    fn row_to_org_invitation(row: &sqlx::postgres::PgRow) -> OrgInvitation {
        OrgInvitation {
            id: row.get("id"),
            org_id: row.get("org_id"),
            invite_email: row.get("invite_email"),
            invite_login: row.get("invite_login"),
            role: row.get("role"),
            status: row.get("status"),
            invited_by: row.get("invited_by"),
            accepted_by: row.get("accepted_by"),
            accepted_at: row.get("accepted_at_ms"),
            revoked_by: row.get("revoked_by"),
            revoked_at: row.get("revoked_at_ms"),
            expires_at: row.get("expires_at_ms"),
            created_at: row.get("created_at_ms"),
            updated_at: row.get("updated_at_ms"),
        }
    }

    pub async fn create_org_invitation(
        &self,
        input: &CreateOrgInvitationInput<'_>,
    ) -> Result<OrgInvitation, DbError> {
        let row = sqlx::query(
            r#"
            INSERT INTO org_invitations (
                org_id,
                invite_email,
                invite_login,
                role,
                token_hash,
                invited_by,
                expires_at
            )
            VALUES ($1::uuid, $2, $3, $4, $5, $6, $7)
            RETURNING
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            "#,
        )
        .bind(input.org_id)
        .bind(input.invite_email)
        .bind(input.invite_login)
        .bind(input.role)
        .bind(input.token_hash)
        .bind(input.invited_by)
        .bind(input.expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(Self::row_to_org_invitation(&row))
    }

    pub async fn list_org_invitations(
        &self,
        org_id: &str,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<OrgInvitation>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                CASE
                    WHEN status = 'pending' AND expires_at < NOW() THEN 'expired'
                    ELSE status
                END AS status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            FROM org_invitations
            WHERE org_id = $1::uuid
              AND (
                    $2::text IS NULL
                    OR (
                        CASE
                            WHEN status = 'pending' AND expires_at < NOW() THEN 'expired'
                            ELSE status
                        END
                    ) = $2
              )
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(org_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM org_invitations
            WHERE org_id = $1::uuid
              AND (
                    $2::text IS NULL
                    OR (
                        CASE
                            WHEN status = 'pending' AND expires_at < NOW() THEN 'expired'
                            ELSE status
                        END
                    ) = $2
              )
            "#,
        )
        .bind(org_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let total: i64 = count_row.get("total");
        let entries = rows.iter().map(Self::row_to_org_invitation).collect();
        Ok((entries, total))
    }

    pub async fn resend_org_invitation(
        &self,
        invitation_id: &str,
        scope_org_id: Option<&str>,
        token_hash: &str,
        actor: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<OrgInvitation>, DbError> {
        let row = sqlx::query(
            r#"
            UPDATE org_invitations
            SET
                token_hash = $3,
                status = 'pending',
                invited_by = $4,
                expires_at = $5,
                accepted_by = NULL,
                accepted_at = NULL,
                revoked_by = NULL,
                revoked_at = NULL,
                updated_at = NOW()
            WHERE id = $1::uuid
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
              AND status <> 'accepted'
            RETURNING
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            "#,
        )
        .bind(invitation_id)
        .bind(scope_org_id)
        .bind(token_hash)
        .bind(actor)
        .bind(expires_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| Self::row_to_org_invitation(&r)))
    }

    pub async fn revoke_org_invitation(
        &self,
        invitation_id: &str,
        scope_org_id: Option<&str>,
        actor: &str,
    ) -> Result<Option<OrgInvitation>, DbError> {
        let row = sqlx::query(
            r#"
            UPDATE org_invitations
            SET
                status = 'revoked',
                revoked_by = $3,
                revoked_at = NOW(),
                updated_at = NOW()
            WHERE id = $1::uuid
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
              AND status = 'pending'
            RETURNING
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            "#,
        )
        .bind(invitation_id)
        .bind(scope_org_id)
        .bind(actor)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| Self::row_to_org_invitation(&r)))
    }

    pub async fn get_org_invitation_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<OrgInvitation>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                CASE
                    WHEN status = 'pending' AND expires_at < NOW() THEN 'expired'
                    ELSE status
                END AS status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            FROM org_invitations
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| Self::row_to_org_invitation(&r)))
    }

    pub async fn accept_org_invitation(
        &self,
        token_hash: &str,
        requested_login: Option<&str>,
    ) -> Result<Option<AcceptedOrgInvitation>, DbError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let invite_row = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            FROM org_invitations
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let Some(invite_row) = invite_row else {
            tx.rollback()
                .await
                .map_err(|e| DbError::DatabaseError(e.to_string()))?;
            return Ok(None);
        };

        let invitation = Self::row_to_org_invitation(&invite_row);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let is_pending = invitation.status == "pending";
        let is_not_expired = invitation.expires_at > now_ms;
        if !is_pending || !is_not_expired {
            tx.rollback()
                .await
                .map_err(|e| DbError::DatabaseError(e.to_string()))?;
            return Ok(None);
        }

        let resolved_login = requested_login
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                invitation
                    .invite_login
                    .clone()
                    .map(|s| s.trim().to_string())
            })
            .or_else(|| {
                invitation
                    .invite_email
                    .as_ref()
                    .and_then(|email| email.split('@').next().map(str::trim))
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
            });

        let Some(login) = resolved_login else {
            tx.rollback()
                .await
                .map_err(|e| DbError::DatabaseError(e.to_string()))?;
            return Err(DbError::DatabaseError(
                "Invitation does not have a resolvable login".to_string(),
            ));
        };

        let existing_id = sqlx::query(
            r#"
            SELECT id::text AS id
            FROM org_users
            WHERE org_id = $1::uuid
              AND login = $2
            "#,
        )
        .bind(&invitation.org_id)
        .bind(&login)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?
        .map(|r| r.get::<String, _>("id"));

        let org_user_row = if let Some(id) = existing_id {
            sqlx::query(
                r#"
                UPDATE org_users
                SET
                    email = COALESCE($2, email),
                    role = $3,
                    status = 'active',
                    updated_by = $4,
                    updated_at = NOW()
                WHERE id = $1::uuid
                RETURNING
                    id::text,
                    org_id::text,
                    login,
                    display_name,
                    email,
                    role,
                    status,
                    created_by,
                    updated_by,
                    EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                    EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
                "#,
            )
            .bind(id)
            .bind(invitation.invite_email.as_deref())
            .bind(&invitation.role)
            .bind(&login)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?
        } else {
            sqlx::query(
                r#"
                INSERT INTO org_users (
                    org_id, login, display_name, email, role, status, created_by, updated_by
                )
                VALUES ($1::uuid, $2, NULL, $3, $4, 'active', $2, $2)
                RETURNING
                    id::text,
                    org_id::text,
                    login,
                    display_name,
                    email,
                    role,
                    status,
                    created_by,
                    updated_by,
                    EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                    EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
                "#,
            )
            .bind(&invitation.org_id)
            .bind(&login)
            .bind(invitation.invite_email.as_deref())
            .bind(&invitation.role)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?
        };
        let org_user = Self::row_to_org_user(&org_user_row);

        let api_key = uuid::Uuid::new_v4().to_string();
        let key_hash = format!("{:x}", sha2::Sha256::digest(api_key.as_bytes()));
        sqlx::query(
            r#"
            INSERT INTO api_keys (key_hash, client_id, org_id, role)
            VALUES ($1, $2, $3::uuid, $4)
            "#,
        )
        .bind(&key_hash)
        .bind(&org_user.login)
        .bind(&invitation.org_id)
        .bind(&invitation.role)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let accepted_row = sqlx::query(
            r#"
            UPDATE org_invitations
            SET
                status = 'accepted',
                accepted_by = $2,
                accepted_at = NOW(),
                updated_at = NOW()
            WHERE id = $1::uuid
            RETURNING
                id::text,
                org_id::text,
                invite_email,
                invite_login,
                role,
                status,
                invited_by,
                accepted_by,
                EXTRACT(EPOCH FROM accepted_at)::bigint * 1000 AS accepted_at_ms,
                revoked_by,
                EXTRACT(EPOCH FROM revoked_at)::bigint * 1000 AS revoked_at_ms,
                EXTRACT(EPOCH FROM expires_at)::bigint * 1000 AS expires_at_ms,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms,
                EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at_ms
            "#,
        )
        .bind(&invitation.id)
        .bind(&org_user.login)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(Some(AcceptedOrgInvitation {
            invitation: Self::row_to_org_invitation(&accepted_row),
            org_user,
            api_key,
        }))
    }

    // ========================================================================
    // CHAT QUERY ENGINE — Conversational MVP
    // ========================================================================

    /// Q1: Pushes to main this week with no Jira ticket correlation.
    pub async fn chat_query_pushes_no_ticket(
        &self,
        org_id: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                ge.actor_login,
                ge.ref_name   AS branch,
                ge.commit_shas::text AS commit_shas,
                EXTRACT(EPOCH FROM ge.created_at)::bigint * 1000 AS event_ts
            FROM github_events ge
            WHERE ge.event_type = 'push'
              AND (ge.ref_name = 'refs/heads/main' OR ge.ref_name = 'main')
              AND ge.created_at >= NOW() - INTERVAL '7 days'
              AND ($1::uuid IS NULL OR ge.org_id = $1::uuid)
              AND NOT EXISTS (
                  SELECT 1
                  FROM commit_ticket_correlations ctc
                  WHERE ctc.commit_sha = ANY(
                      SELECT jsonb_array_elements_text(ge.commit_shas::jsonb)
                  )
              )
            ORDER BY ge.created_at DESC
            LIMIT 50
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "actor": r.get::<Option<String>, _>("actor_login").unwrap_or_default(),
                    "branch": r.get::<Option<String>, _>("branch").unwrap_or_default(),
                    "timestamp": r.get::<i64, _>("event_ts"),
                })
            })
            .collect())
    }

    /// Q1a: Count pushes to main in the last 7 days with no Jira correlation.
    pub async fn chat_query_pushes_no_ticket_count(
        &self,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM github_events ge
            WHERE ge.event_type = 'push'
              AND (ge.ref_name = 'refs/heads/main' OR ge.ref_name = 'main')
              AND ge.created_at >= NOW() - INTERVAL '7 days'
              AND ($1::uuid IS NULL OR ge.org_id = $1::uuid)
              AND NOT EXISTS (
                  SELECT 1
                  FROM commit_ticket_correlations ctc
                  WHERE ctc.commit_sha = ANY(
                      SELECT jsonb_array_elements_text(ge.commit_shas::jsonb)
                  )
              )
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q1b: Count commits without Jira correlation in a recent time window.
    pub async fn chat_query_commits_without_ticket_count(
        &self,
        org_id: Option<&str>,
        hours: i64,
    ) -> Result<i64, DbError> {
        let safe_hours = hours.clamp(1, 24 * 30) as i32;
        let row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT c.commit_sha)::bigint AS cnt
            FROM client_events c
            LEFT JOIN commit_ticket_correlations ct ON ct.commit_sha = c.commit_sha
            WHERE c.event_type = 'commit'
              AND c.commit_sha IS NOT NULL
              AND c.created_at >= NOW() - make_interval(hours => $1::int)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
              AND ct.id IS NULL
            "#,
        )
        .bind(safe_hours)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q1c: Count developers considered online by recent client heartbeat/activity.
    pub async fn chat_query_online_developers_count(
        &self,
        org_id: Option<&str>,
        minutes: i64,
    ) -> Result<i64, DbError> {
        let safe_minutes = minutes.clamp(1, 24 * 60) as i32;
        let row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT client_id)::bigint AS cnt
            FROM client_sessions
            WHERE last_seen_at >= NOW() - make_interval(mins => $1::int)
              AND ($2::uuid IS NULL OR org_id = $2::uuid)
            "#,
        )
        .bind(safe_minutes)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q2: Count blocked pushes this calendar month.
    pub async fn chat_query_blocked_pushes_month(
        &self,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM client_events
            WHERE event_type IN ('blocked_push', 'push_failed', 'attempt_push')
              AND status IN ('blocked', 'failed')
              AND created_at >= date_trunc('month', NOW())
              AND ($1::uuid IS NULL OR org_id = $1::uuid)
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q2b: Count blocked pushes this calendar month for a specific user (alias-aware).
    pub async fn chat_query_user_blocked_pushes_month(
        &self,
        user_login: &str,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM client_events c
            LEFT JOIN identity_aliases ica
              ON ica.alias_login = c.user_login
             AND ($2::uuid IS NULL OR ica.org_id = $2::uuid)
            WHERE c.event_type IN ('blocked_push', 'push_failed', 'attempt_push')
              AND c.status IN ('blocked', 'failed')
              AND c.created_at >= date_trunc('month', NOW())
              AND (c.user_login ILIKE $1 OR COALESCE(ica.canonical_login, c.user_login) ILIKE $1)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
            "#,
        )
        .bind(user_login)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q2c: Count successful pushes for a specific user, optionally scoped to a time window.
    pub async fn chat_query_user_pushes_count(
        &self,
        user_login: &str,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM client_events c
            LEFT JOIN identity_aliases ica
              ON ica.alias_login = c.user_login
             AND ($4::uuid IS NULL OR ica.org_id = $4::uuid)
            WHERE c.event_type = 'successful_push'
              AND (c.user_login ILIKE $1 OR COALESCE(ica.canonical_login, c.user_login) ILIKE $1)
              AND ($2::bigint IS NULL OR c.created_at >= to_timestamp($2::bigint / 1000.0))
              AND ($3::bigint IS NULL OR c.created_at <= to_timestamp($3::bigint / 1000.0))
              AND ($4::uuid IS NULL OR c.org_id = $4::uuid)
            "#,
        )
        .bind(user_login)
        .bind(start_ms)
        .bind(end_ms)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q1b: Count pushes to main this week with no Jira ticket for a specific user (alias-aware).
    pub async fn chat_query_user_pushes_no_ticket_week(
        &self,
        user_login: &str,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM github_events ge
            LEFT JOIN identity_aliases iga
              ON iga.alias_login = ge.actor_login
             AND ($2::uuid IS NULL OR iga.org_id = $2::uuid)
            WHERE ge.event_type = 'push'
              AND (ge.ref_name = 'refs/heads/main' OR ge.ref_name = 'main')
              AND ge.created_at >= NOW() - INTERVAL '7 days'
              AND (ge.actor_login ILIKE $1 OR COALESCE(iga.canonical_login, ge.actor_login) ILIKE $1)
              AND ($2::uuid IS NULL OR ge.org_id = $2::uuid)
              AND NOT EXISTS (
                  SELECT 1
                  FROM commit_ticket_correlations ctc
                  WHERE ctc.commit_sha = ANY(
                      SELECT jsonb_array_elements_text(ge.commit_shas::jsonb)
                  )
              )
            "#,
        )
        .bind(user_login)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q3: Commits by a specific user in a time range [start_ms, end_ms] (epoch millis).
    pub async fn chat_query_user_commits_range(
        &self,
        user_login: &str,
        start_ms: i64,
        end_ms: i64,
        org_id: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(ica.canonical_login, c.user_login) AS canonical_user_login,
                c.branch,
                c.commit_sha,
                (EXTRACT(EPOCH FROM c.created_at) * 1000)::bigint AS event_ts
            FROM client_events c
            LEFT JOIN identity_aliases ica
              ON ica.alias_login = c.user_login
             AND ($4::uuid IS NULL OR ica.org_id = $4::uuid)
            WHERE c.event_type = 'commit'
              AND (c.user_login ILIKE $1 OR COALESCE(ica.canonical_login, c.user_login) ILIKE $1)
              AND c.created_at >= to_timestamp($2::bigint / 1000.0)
              AND c.created_at <= to_timestamp($3::bigint / 1000.0)
              AND ($4::uuid IS NULL OR c.org_id = $4::uuid)
            ORDER BY c.created_at DESC
            LIMIT 100
            "#,
        )
        .bind(user_login)
        .bind(start_ms)
        .bind(end_ms)
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "user_login": r.get::<String, _>("canonical_user_login"),
                    "branch": r.get::<Option<String>, _>("branch").unwrap_or_default(),
                    "commit_sha": r.get::<Option<String>, _>("commit_sha").unwrap_or_default(),
                    "timestamp": r.get::<i64, _>("event_ts"),
                })
            })
            .collect())
    }

    /// Q4: Count commits by a specific user, optionally scoped to a time window.
    pub async fn chat_query_user_commits_count(
        &self,
        user_login: &str,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM client_events c
            LEFT JOIN identity_aliases ica
              ON ica.alias_login = c.user_login
             AND ($4::uuid IS NULL OR ica.org_id = $4::uuid)
            WHERE c.event_type = 'commit'
              AND (c.user_login ILIKE $1 OR COALESCE(ica.canonical_login, c.user_login) ILIKE $1)
              AND ($2::bigint IS NULL OR c.created_at >= to_timestamp($2::bigint / 1000.0))
              AND ($3::bigint IS NULL OR c.created_at <= to_timestamp($3::bigint / 1000.0))
              AND ($4::uuid IS NULL OR c.org_id = $4::uuid)
            "#,
        )
        .bind(user_login)
        .bind(start_ms)
        .bind(end_ms)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Q4b: Latest commit metadata by user in current scope (alias-aware).
    pub async fn chat_query_user_last_commit(
        &self,
        user_login: &str,
        org_id: Option<&str>,
    ) -> Result<Option<serde_json::Value>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(ica.canonical_login, c.user_login) AS canonical_user_login,
                c.user_name,
                c.event_uuid,
                c.branch,
                c.commit_sha,
                r.full_name AS repo_full_name,
                COALESCE(c.metadata->>'commit_message', c.metadata->>'message') AS commit_message,
                (EXTRACT(EPOCH FROM c.created_at) * 1000)::bigint AS event_ts
            FROM client_events c
            LEFT JOIN repos r ON c.repo_id = r.id
            LEFT JOIN identity_aliases ica
              ON ica.alias_login = c.user_login
             AND ($2::uuid IS NULL OR ica.org_id = $2::uuid)
            WHERE c.event_type = 'commit'
              AND (c.user_login ILIKE $1 OR COALESCE(ica.canonical_login, c.user_login) ILIKE $1)
              AND ($2::uuid IS NULL OR c.org_id = $2::uuid)
            ORDER BY c.created_at DESC, c.id DESC
            LIMIT 1
            "#,
        )
        .bind(user_login)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| {
            serde_json::json!({
                "user_login": r.get::<String, _>("canonical_user_login"),
                "user_name": r.get::<Option<String>, _>("user_name"),
                "event_uuid": r.get::<String, _>("event_uuid"),
                "branch": r.get::<Option<String>, _>("branch"),
                "commit_sha": r.get::<Option<String>, _>("commit_sha"),
                "repo_full_name": r.get::<Option<String>, _>("repo_full_name"),
                "commit_message": r.get::<Option<String>, _>("commit_message"),
                "timestamp": r.get::<i64, _>("event_ts"),
            })
        }))
    }

    /// Access profile by user login in org scope (never returns plaintext API key values).
    pub async fn chat_query_user_access_profile(
        &self,
        user_login: &str,
        org_id: Option<&str>,
    ) -> Result<Option<serde_json::Value>, DbError> {
        let row = sqlx::query(
            r#"
            SELECT
                ou.login,
                ou.role,
                ou.status,
                EXISTS (
                    SELECT 1
                    FROM api_keys ak
                    WHERE ak.client_id = ou.login
                      AND ak.is_active = TRUE
                      AND ($2::uuid IS NULL OR ak.org_id = $2::uuid)
                ) AS has_active_api_key
            FROM org_users ou
            WHERE ou.login ILIKE $1
              AND ($2::uuid IS NULL OR ou.org_id = $2::uuid)
            LIMIT 1
            "#,
        )
        .bind(user_login)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| {
            serde_json::json!({
                "login": r.get::<String, _>("login"),
                "role": r.get::<String, _>("role"),
                "status": r.get::<String, _>("status"),
                "has_active_api_key": r.get::<bool, _>("has_active_api_key"),
            })
        }))
    }

    /// Q5: Count commits in an optional time window, optionally scoped by org.
    pub async fn chat_query_commits_count(
        &self,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        org_id: Option<&str>,
    ) -> Result<i64, DbError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*)::bigint AS cnt
            FROM client_events
            WHERE event_type = 'commit'
              AND ($1::bigint IS NULL OR created_at >= to_timestamp($1::bigint / 1000.0))
              AND ($2::bigint IS NULL OR created_at <= to_timestamp($2::bigint / 1000.0))
              AND ($3::uuid IS NULL OR org_id = $3::uuid)
            "#,
        )
        .bind(start_ms)
        .bind(end_ms)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("cnt"))
    }

    /// Insert a feature request record. Returns the new UUID as String.
    pub async fn create_feature_request(
        &self,
        input: &crate::models::FeatureRequestInput,
        requested_by: &str,
    ) -> Result<String, DbError> {
        let metadata = input
            .metadata
            .as_ref()
            .cloned()
            .unwrap_or(serde_json::Value::Object(Default::default()));

        let row = sqlx::query(
            r#"
            INSERT INTO feature_requests
                (org_id, requested_by, question, missing_capability, metadata)
            VALUES ($1::uuid, $2, $3, $4, $5)
            RETURNING id::text
            "#,
        )
        .bind(input.org_id.as_deref())
        .bind(requested_by)
        .bind(&input.question)
        .bind(input.missing_capability.as_deref())
        .bind(&metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(row.get("id"))
    }

    // ========================================================================
    // CLI COMMAND AUDIT
    // ========================================================================

    pub async fn insert_cli_command(
        &self,
        record: &crate::models::CliCommandRecord,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO cli_commands (
                id, org_id, user_login, command, origin, branch,
                repo_name, exit_code, duration_ms, metadata, created_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3, $4, $5, $6,
                $7, $8, $9, $10::jsonb, to_timestamp($11::bigint / 1000.0)
            )
            "#,
        )
        .bind(&record.id)
        .bind(&record.org_id)
        .bind(&record.user_login)
        .bind(&record.command)
        .bind(&record.origin)
        .bind(&record.branch)
        .bind(&record.repo_name)
        .bind(record.exit_code)
        .bind(record.duration_ms)
        .bind(&record.metadata)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn list_cli_commands(
        &self,
        org_id: Option<&str>,
        user_login: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::models::CliCommandRecord>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text, org_id::text, user_login, command, origin, branch,
                repo_name, exit_code, duration_ms,
                COALESCE(metadata, '{}'::jsonb) AS metadata,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM cli_commands
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
              AND ($2::text IS NULL OR user_login = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(org_id)
        .bind(user_login)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM cli_commands
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
              AND ($2::text IS NULL OR user_login = $2)
            "#,
        )
        .bind(org_id)
        .bind(user_login)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let records: Vec<crate::models::CliCommandRecord> = rows
            .iter()
            .map(|row| crate::models::CliCommandRecord {
                id: row.get("id"),
                org_id: row.get("org_id"),
                user_login: row.get("user_login"),
                command: row.get("command"),
                origin: row.get("origin"),
                branch: row.get("branch"),
                repo_name: row.get("repo_name"),
                exit_code: row.get("exit_code"),
                duration_ms: row.get("duration_ms"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at_ms"),
            })
            .collect();

        Ok((records, count))
    }

    // ========================================================================
    // POLICY DRIFT AUDIT
    // ========================================================================

    pub async fn insert_policy_drift_event(
        &self,
        record: &crate::models::PolicyDriftEventRecord,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO policy_drift_events (
                id, org_id, user_login, action, repo_name, result,
                before_checksum, after_checksum, duration_ms, metadata, created_at
            )
            VALUES (
                $1::uuid, $2::uuid, $3, $4, $5, $6,
                $7, $8, $9, $10::jsonb, to_timestamp($11::bigint / 1000.0)
            )
            "#,
        )
        .bind(&record.id)
        .bind(&record.org_id)
        .bind(&record.user_login)
        .bind(&record.action)
        .bind(&record.repo_name)
        .bind(&record.result)
        .bind(&record.before_checksum)
        .bind(&record.after_checksum)
        .bind(record.duration_ms)
        .bind(&record.metadata)
        .bind(record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn list_policy_drift_events(
        &self,
        org_id: Option<&str>,
        user_login: Option<&str>,
        repo_name: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::models::PolicyDriftEventRecord>, i64), DbError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id::text,
                org_id::text,
                user_login,
                action,
                repo_name,
                result,
                before_checksum,
                after_checksum,
                duration_ms,
                COALESCE(metadata, '{}'::jsonb) AS metadata,
                EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at_ms
            FROM policy_drift_events
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
              AND ($2::text IS NULL OR user_login = $2)
              AND ($3::text IS NULL OR repo_name = $3)
            ORDER BY created_at DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(org_id)
        .bind(user_login)
        .bind(repo_name)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM policy_drift_events
            WHERE ($1::uuid IS NULL OR org_id = $1::uuid)
              AND ($2::text IS NULL OR user_login = $2)
              AND ($3::text IS NULL OR repo_name = $3)
            "#,
        )
        .bind(org_id)
        .bind(user_login)
        .bind(repo_name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let records: Vec<crate::models::PolicyDriftEventRecord> = rows
            .iter()
            .map(|row| crate::models::PolicyDriftEventRecord {
                id: row.get("id"),
                org_id: row.get("org_id"),
                user_login: row.get("user_login"),
                action: row.get("action"),
                repo_name: row.get("repo_name"),
                result: row.get("result"),
                before_checksum: row.get("before_checksum"),
                after_checksum: row.get("after_checksum"),
                duration_ms: row.get("duration_ms"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at_ms"),
            })
            .collect();

        Ok((records, count))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

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
        simulate_auth_db_failure: Option<String>,
        simulate_auth_db_failure_flag_file: Option<String>,
    }

    impl EnvGuard {
        fn apply(simulate_auth_db_failure: &str, flag_file: Option<&std::path::Path>) -> Self {
            let guard = Self {
                simulate_auth_db_failure: std::env::var(SIMULATE_AUTH_DB_FAILURE_ENV).ok(),
                simulate_auth_db_failure_flag_file: std::env::var(
                    SIMULATE_AUTH_DB_FAILURE_FLAG_FILE_ENV,
                )
                .ok(),
            };
            set_env_var(SIMULATE_AUTH_DB_FAILURE_ENV, simulate_auth_db_failure);
            match flag_file {
                Some(path) => set_env_var(
                    SIMULATE_AUTH_DB_FAILURE_FLAG_FILE_ENV,
                    &path.as_os_str().to_string_lossy(),
                ),
                None => remove_env_var(SIMULATE_AUTH_DB_FAILURE_FLAG_FILE_ENV),
            }
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            set_or_clear_env(
                SIMULATE_AUTH_DB_FAILURE_ENV,
                self.simulate_auth_db_failure.as_deref(),
            );
            set_or_clear_env(
                SIMULATE_AUTH_DB_FAILURE_FLAG_FILE_ENV,
                self.simulate_auth_db_failure_flag_file.as_deref(),
            );
        }
    }

    struct TempFileGuard {
        path: std::path::PathBuf,
    }

    impl TempFileGuard {
        fn create() -> Self {
            let path = std::env::temp_dir().join(format!(
                "gitgov-auth-db-failpoint-{}.flag",
                uuid::Uuid::new_v4()
            ));
            std::fs::write(&path, b"1").expect("failed to create temp failpoint flag file");
            Self { path }
        }
    }

    impl Drop for TempFileGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn auth_db_failure_simulation_enabled_reads_bool_env() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvGuard::apply("true", None);
        assert!(auth_db_failure_simulation_enabled());
    }

    #[test]
    fn auth_db_failure_simulation_enabled_reads_flag_file() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        let flag = TempFileGuard::create();
        let _env_guard = EnvGuard::apply("false", Some(&flag.path));
        assert!(auth_db_failure_simulation_enabled());
    }

    fn build_test_db(
        auth_cache_ttl_secs: u64,
        auth_cache_stale_max_secs: u64,
        auth_stale_fail_closed_after: u32,
    ) -> Database {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://gitgov:gitgov@127.0.0.1/gitgov")
            .expect("failed to build lazy pg pool for auth cache tests");
        Database {
            pool,
            auth_cache: std::sync::Arc::new(
                std::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            auth_cache_ttl: Duration::from_secs(auth_cache_ttl_secs),
            auth_cache_stale_max: Duration::from_secs(auth_cache_stale_max_secs),
            auth_cache_max_entries: 64,
            auth_db_failure_streak: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            auth_stale_fail_closed_after,
        }
    }

    #[tokio::test]
    async fn expired_fresh_cache_entry_remains_available_for_stale_lookup() {
        let db = build_test_db(1, 120, 0);
        db.put_cached_api_key_auth(
            "k",
            Some((
                "admin".to_string(),
                UserRole::Admin,
                Some("org1".to_string()),
            )),
        );

        {
            let mut cache = db.auth_cache.lock().expect("auth cache poisoned");
            let entry = cache.get_mut("k").expect("missing cached entry");
            entry.cached_at = Instant::now() - Duration::from_secs(2);
        }

        assert!(db.get_cached_api_key_auth("k").is_none());

        let stale = db
            .get_stale_cached_api_key_auth("k")
            .expect("stale auth cache should be available");
        assert_eq!(stale.0 .0, "admin");
        assert!(stale.1 >= 1);
    }

    #[tokio::test]
    async fn stale_cache_entry_older_than_max_age_is_evicted() {
        let db = build_test_db(1, 2, 0);
        db.put_cached_api_key_auth(
            "k",
            Some((
                "admin".to_string(),
                UserRole::Admin,
                Some("org1".to_string()),
            )),
        );

        {
            let mut cache = db.auth_cache.lock().expect("auth cache poisoned");
            let entry = cache.get_mut("k").expect("missing cached entry");
            entry.cached_at = Instant::now() - Duration::from_secs(3);
        }

        assert!(db.get_cached_api_key_auth("k").is_none());
        assert!(db.get_stale_cached_api_key_auth("k").is_none());
        let cache = db.auth_cache.lock().expect("auth cache poisoned");
        assert!(cache.get("k").is_none());
    }

    #[tokio::test]
    async fn auth_db_failure_threshold_trips_fail_closed_mode() {
        let db = build_test_db(1, 120, 3);
        let (streak1, fail_closed1) = db.note_auth_db_failure();
        let (streak2, fail_closed2) = db.note_auth_db_failure();
        let (streak3, fail_closed3) = db.note_auth_db_failure();

        assert_eq!(streak1, 1);
        assert_eq!(streak2, 2);
        assert_eq!(streak3, 3);
        assert!(!fail_closed1);
        assert!(!fail_closed2);
        assert!(fail_closed3);
    }

    #[tokio::test]
    async fn auth_db_failure_threshold_zero_keeps_stale_enabled() {
        let db = build_test_db(1, 120, 0);
        for _ in 0..5 {
            let (_, fail_closed) = db.note_auth_db_failure();
            assert!(!fail_closed);
        }
    }

    #[tokio::test]
    async fn auth_db_failure_streak_resets_after_success_signal() {
        let db = build_test_db(1, 120, 2);
        let (_, fail_closed1) = db.note_auth_db_failure();
        assert!(!fail_closed1);

        db.reset_auth_db_failure_streak();

        let (streak_after_reset, fail_closed_after_reset) = db.note_auth_db_failure();
        assert_eq!(streak_after_reset, 1);
        assert!(!fail_closed_after_reset);
    }
}
