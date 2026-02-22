use crate::models::{AuditAction, AuditFilter, AuditLogEntry, AuditStats, AuditStatus};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Lock error")]
    LockError,
}

pub struct AuditDatabase {
    conn: Mutex<Connection>,
}

impl AuditDatabase {
    pub fn new(db_path: &str) -> Result<Self, AuditError> {
        let path = Path::new(db_path);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AuditError::DatabaseError(format!("Failed to create directory: {}", e))
                })?;
            }
        }

        let conn = Connection::open(path).map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                developer_login TEXT NOT NULL,
                developer_name TEXT NOT NULL,
                action TEXT NOT NULL,
                branch TEXT NOT NULL,
                files TEXT NOT NULL,
                commit_hash TEXT,
                status TEXT NOT NULL,
                reason TEXT
            );
            
            CREATE INDEX IF NOT EXISTS idx_developer_login ON audit_logs(developer_login);
            CREATE INDEX IF NOT EXISTS idx_timestamp ON audit_logs(timestamp);
            CREATE INDEX IF NOT EXISTS idx_action ON audit_logs(action);
            CREATE INDEX IF NOT EXISTS idx_status ON audit_logs(status);
            "#,
        )
        .map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert(&self, entry: &AuditLogEntry) -> Result<(), AuditError> {
        let files_json = serde_json::to_string(&entry.files)
            .map_err(|e| AuditError::SerializationError(e.to_string()))?;

        let conn = self.conn.lock().map_err(|_| AuditError::LockError)?;

        conn.execute(
            r#"
            INSERT INTO audit_logs (id, timestamp, developer_login, developer_name, action, branch, files, commit_hash, status, reason)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                entry.id,
                entry.timestamp,
                entry.developer_login,
                entry.developer_name,
                serde_json::to_string(&entry.action).unwrap_or_default(),
                entry.branch,
                files_json,
                entry.commit_hash,
                serde_json::to_string(&entry.status).unwrap_or_default(),
                entry.reason,
            ],
        )
        .map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditLogEntry>, AuditError> {
        let conn = self.conn.lock().map_err(|_| AuditError::LockError)?;

        let mut query = String::from("SELECT id, timestamp, developer_login, developer_name, action, branch, files, commit_hash, status, reason FROM audit_logs WHERE 1=1");
        let mut bind_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(start) = filter.start_date {
            query.push_str(" AND timestamp >= ?");
            bind_params.push(Box::new(start));
        }

        if let Some(end) = filter.end_date {
            query.push_str(" AND timestamp <= ?");
            bind_params.push(Box::new(end));
        }

        if let Some(ref login) = filter.developer_login {
            query.push_str(" AND developer_login = ?");
            bind_params.push(Box::new(login.clone()));
        }

        if let Some(ref action) = filter.action {
            query.push_str(" AND action = ?");
            bind_params.push(Box::new(serde_json::to_string(action).unwrap_or_default()));
        }

        if let Some(ref status) = filter.status {
            query.push_str(" AND status = ?");
            bind_params.push(Box::new(serde_json::to_string(status).unwrap_or_default()));
        }

        if let Some(ref branch) = filter.branch {
            query.push_str(" AND branch = ?");
            bind_params.push(Box::new(branch.clone()));
        }

        query.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        bind_params.push(Box::new(filter.limit as i64));
        bind_params.push(Box::new(filter.offset as i64));

        let params: Vec<&dyn rusqlite::ToSql> = bind_params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        let entries = stmt
            .query_map(params.as_slice(), |row| {
                let action_str: String = row.get(4)?;
                let status_str: String = row.get(8)?;
                let files_json: String = row.get(6)?;

                Ok(AuditLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    developer_login: row.get(2)?,
                    developer_name: row.get(3)?,
                    action: serde_json::from_str(&action_str).unwrap_or(AuditAction::Push),
                    branch: row.get(5)?,
                    files: serde_json::from_str(&files_json).unwrap_or_default(),
                    commit_hash: row.get(7)?,
                    status: serde_json::from_str(&status_str).unwrap_or(AuditStatus::Failed),
                    reason: row.get(9)?,
                })
            })
            .map_err(|e| AuditError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        Ok(entries)
    }

    pub fn get_stats(&self) -> Result<AuditStats, AuditError> {
        let conn = self.conn.lock().map_err(|_| AuditError::LockError)?;

        let now = chrono::Utc::now();
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .timestamp_millis();
        let week_start = (now - chrono::Duration::days(7)).timestamp_millis();

        let pushes_today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_logs WHERE action = ? AND timestamp >= ?",
                params![
                    serde_json::to_string(&AuditAction::Push).unwrap_or_default(),
                    today_start
                ],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let blocked_today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_logs WHERE status = ? AND timestamp >= ?",
                params![
                    serde_json::to_string(&AuditStatus::Blocked).unwrap_or_default(),
                    today_start
                ],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let active_devs_this_week: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT developer_login) FROM audit_logs WHERE timestamp >= ?",
                params![week_start],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let most_frequent_action: Option<String> = conn
            .query_row(
                "SELECT action FROM audit_logs GROUP BY action ORDER BY COUNT(*) DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok()
            .and_then(|s: String| serde_json::from_str(&s).ok());

        Ok(AuditStats {
            pushes_today,
            blocked_today,
            active_devs_this_week,
            most_frequent_action,
        })
    }
}
