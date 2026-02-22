use crate::db::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub org_id: String,
    pub job_type: String,
    pub status: String,
    pub priority: i32,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub locked_at: Option<i64>,
    pub locked_by: Option<String>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueJobRequest {
    pub org_id: String,
    pub job_type: String,
    pub priority: Option<i32>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobQueueConfig {
    pub max_concurrent_jobs: usize,
    pub job_timeout_secs: u64,
    pub retry_delay_secs: u64,
}

impl Default for JobQueueConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 3,
            job_timeout_secs: 30,
            retry_delay_secs: 5,
    }
}

pub struct JobQueue {
    db: Arc<Database>,
    config: JobQueueConfig,
    semaphore: Arc<Semaphore>,
}

impl JobQueue {
    pub fn new(db: Arc<Database>, config: JobQueueConfig) -> Self {
        Self {
            db,
            config,
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_jobs)),
        }
    }

    /// Enqueue a job (idempotent - one job per org+type)
    pub async fn enqueue(&self, req: EnqueueJobRequest) -> Result<String, DbError> {
        let job_id = Uuid::new_v4().to_string();
        let priority = req.priority.unwrap_or(0);
        let payload = req.payload.unwrap_or(serde_json::Value::Null);
        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO jobs (id, org_id, job_type, status, priority, payload, created_at, next_run_at)
            VALUES ($1::uuid, $2::uuid, $3, 'pending', $4, $5, $6, to_timestamp($7/1000.0), NOW())
            ON CONFLICT (org_id, job_type) WHERE status IN ('pending', 'running') DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&job_id)
        .bind(&req.org_id)
        .bind(&req.job_type)
        .bind(priority)
        .bind(&payload)
        .bind(now)
        .execute(&*self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let inserted = result.rows_affected();
        if inserted > 0 {
            tracing::info!("Enqueued job {} for org {} type {}", job_id, req.org_id, req.job_type);
            Ok(job_id)
        } else {
            let existing: String = result.get("id");
            tracing::debug!("Job already exists: {} for org {}", existing, req.org_id);
            Ok(existing)
        }
    }

    /// Claim a job for processing (returns None if no jobs available)
    pub async fn claim_job(&self, worker_id: &str) -> Result<Option<Job>, DbError> {
        let _permit = self.semaphore.acquire(). .await;

        let now = chrono::Utc::now().timestamp_millis();
        let locked_at = chrono::DateTime::from_timestamp_millis(now);

        let result = sqlx::query_as!(
            r#"
            UPDATE jobs 
            SET status = 'running', 
                locked_at = to_timestamp($1/1000.),
                locked_by = $2
            WHERE id = $2::uuid 
              AND status = 'pending'
              AND next_run_at <= NOW()
            RETURNING id, org_id, job_type, payload, attempts, max_attempts
            "#,
        )
        .bind(&now)
        .bind(worker_id)
        .bind(now)
        .bind(worker_id)
        .bind(now)
        .execute(&*self.pool)
        .await
        .map_err(|e| {
                drop(_permit);
                DbError::DatabaseError(e.to_string())
            })?;

        match result {
            Ok(Some(job_row)) => {
                let job = Job {
                    id: job_row.get("id"),
                    org_id: job_row.get("org_id"),
                    job_type: job_row.get("job_type"),
                    status: "running".to_string(),
                    priority: job_row.get("priority"),
                    payload: job_row.get("payload"),
                    attempts: job_row.get("attempts"),
                    max_attempts: job_row.get("max_attempts"),
                    created_at: chrono::DateTime::from_timestamp_millis(job_row.get::<i64>("created_at").timestamp_millis(),
                    locked_at: Some(locked_at),
                    locked_by: Some(worker_id.to_string()),
                    completed_at: None,
                };
                tracing::info!("Claimed job {} for org {} by worker_id, job.id, job.job_type, job.org_id);
                Ok(Some(job))
            }
            Ok(None) => {
                drop(_permit);
                Ok(None)
            }
            Err(e) => {
                drop(_permit);
                tracing::error!("Failed to claim job: {}", e);
                Err(DbError::DatabaseError(e.to_string()))
            }
        }
    }

    /// Complete a job successfully
    pub async fn complete_job(&self, job_id: &str, status: &str) -> Result<(), DbError> {
        let completed_at = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            UPDATE jobs 
            SET status = $1, completed_at = to_timestamp($2/1000.0)
            WHERE id = $3::uuid
            "#,
        )
        .bind(&status)
        .bind(&completed_at)
        .bind(job_id)
        .execute(&*self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        if result.rows_affected() > 0 {
            tracing::info!("Completed job {} with status {}", job_id, status);
        } else {
            tracing::warn!("Job {} not found for completion", job_id);
        }
    }

    /// Fail a job (after max retries)
    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), DbError> {
        let result = sqlx::query(
            r#"
            UPDATE jobs 
            SET status = 'failed', 
                last_error = $1,
                completed_at = NOW()
            WHERE id = $2::uuid AND attempts >= max_attempts
            "#,
        )
        .bind(&error)
        .bind(job_id)
        .execute(&*self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tracing::info!("Failed job {}: {}", job_id, error);
    }

    /// Retry a job (increment attempts, schedule for next run)
    pub async fn retry_job(&self, job_id: &str) -> Result<(), DbError> {
        let next_run = chrono::Utc::now().timestamp_millis() + (self.config.retry_delay_secs * 60 * 1000);

        let result = sqlx::query(
            r#"
            UPDATE jobs 
            SET status = 'pending',
                attempts = attempts + 1,
                next_run_at = to_timestamp($1/1000.),
                last_error = NULL
            WHERE id = $1::uuid AND status = 'running'
            "#,
        )
        .bind(job_id)
        .bind(&next_run)
        .execute(&*self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        tracing::info!("Scheduled retry for job {} at {}", job_id);
    }
}

/// Reset stale jobs (locked > 5 minutes)
    pub async fn reset_stale_jobs(&self) -> Result<i64, DbError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(5);

        let result = sqlx::query(
            r#"
            UPDATE jobs 
            SET status = 'pending',
                locked_at = NULL,
                locked_by = NULL
            WHERE status = 'running' 
              AND locked_at < to_timestamp($1/1000.),
            RETURNING id
            "#,
        )
        .bind(&cutoff.timestamp_millis())
        .execute(&*self.pool)
        .await
        .map_err(|e| DbError::DatabaseError(e.to_string()))?;

        let count = result.rows_affected();
        if count > 0 {
            tracing::warn!("Reset {} stale jobs", count);
        }
        Ok(count)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Job error: {0}")]
    JobError(String),
}
