use crate::models::{AuditAction, AuditLogEntry, AuditStatus, GitGovConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Server error: {0}")]
    ServerError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub event_type: String,
    pub timestamp: i64,
    pub developer_login: String,
    pub developer_name: String,
    pub action: AuditAction,
    pub branch: String,
    pub files: Vec<String>,
    pub commit_hash: Option<String>,
    pub status: AuditStatus,
    pub reason: Option<String>,
    pub repo_name: Option<String>,
    pub repo_owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponse {
    pub id: String,
    pub received: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStats {
    pub pushes_today: i64,
    pub blocked_today: i64,
    pub active_devs_this_week: i64,
    pub most_frequent_action: Option<String>,
    pub total_events: i64,
    pub events_by_repo: std::collections::HashMap<String, i64>,
    pub events_by_developer: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResponse {
    pub version: String,
    pub checksum: String,
    pub config: GitGovConfig,
    pub updated_at: i64,
}

pub struct ControlPlaneClient {
    config: ServerConfig,
    client: reqwest::blocking::Client,
}

impl ControlPlaneClient {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn send_event(&self, payload: &EventPayload) -> Result<EventResponse, ServerError> {
        let url = format!("{}/events", self.config.url);

        let mut request = self.client.post(&url).json(payload);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ServerError::ServerError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn get_logs(&self, filter: &AuditFilter) -> Result<Vec<AuditLogEntry>, ServerError> {
        let url = format!("{}/logs", self.config.url);

        let mut request = self.client.get(&url).query(filter);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ServerError::ServerError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct LogsResponse {
            logs: Vec<AuditLogEntry>,
        }

        let result: LogsResponse = response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;

        Ok(result.logs)
    }

    pub fn get_stats(&self) -> Result<ServerStats, ServerError> {
        let url = format!("{}/stats", self.config.url);

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ServerError::ServerError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn get_policy(&self, repo_name: &str) -> Result<Option<PolicyResponse>, ServerError> {
        let url = format!("{}/policy/{}", self.config.url, repo_name);

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(ServerError::ServerError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct PolicyApiResponse {
            version: Option<String>,
            checksum: Option<String>,
            config: Option<GitGovConfig>,
            updated_at: Option<i64>,
        }

        let result: PolicyApiResponse = response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;

        match (
            result.version,
            result.checksum,
            result.config,
            result.updated_at,
        ) {
            (Some(v), Some(c), Some(cfg), Some(u)) => Ok(Some(PolicyResponse {
                version: v,
                checksum: c,
                config: cfg,
                updated_at: u,
            })),
            _ => Ok(None),
        }
    }

    pub fn health_check(&self) -> Result<bool, ServerError> {
        let url = format!("{}/health", self.config.url);

        let response = self
            .client
            .get(&url)
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        Ok(response.status().is_success())
    }
}

impl EventPayload {
    pub fn from_audit_entry(
        entry: &AuditLogEntry,
        repo_name: Option<String>,
        repo_owner: Option<String>,
    ) -> Self {
        Self {
            event_type: "audit".to_string(),
            timestamp: entry.timestamp,
            developer_login: entry.developer_login.clone(),
            developer_name: entry.developer_name.clone(),
            action: entry.action.clone(),
            branch: entry.branch.clone(),
            files: entry.files.clone(),
            commit_hash: entry.commit_hash.clone(),
            status: entry.status.clone(),
            reason: entry.reason.clone(),
            repo_name,
            repo_owner,
        }
    }
}
