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
pub struct CombinedEvent {
    pub id: String,
    pub source: String,
    pub event_type: String,
    pub created_at: i64,
    pub user_login: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuditFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for AuditFilter {
    fn default() -> Self {
        Self {
            source: None,
            start_date: None,
            end_date: None,
            user_login: None,
            developer_login: None,
            event_type: None,
            action: None,
            status: None,
            branch: None,
            repo_full_name: None,
            repo_name: None,
            org_name: None,
            limit: 50,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStats {
    pub github_events: GitHubEventStats,
    pub client_events: ClientEventStats,
    pub violations: ViolationStats,
    pub active_devs_week: i64,
    pub active_repos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubEventStats {
    pub total: i64,
    pub today: i64,
    pub pushes_today: i64,
    #[serde(default)]
    pub by_type: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientEventStats {
    pub total: i64,
    pub today: i64,
    pub blocked_today: i64,
    #[serde(default)]
    pub by_type: std::collections::HashMap<String, i64>,
    #[serde(default)]
    pub by_status: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViolationStats {
    pub total: i64,
    pub unresolved: i64,
    pub critical: i64,
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

    fn endpoint_url(&self, segments: &[&str]) -> Result<reqwest::Url, ServerError> {
        let mut url = reqwest::Url::parse(&self.config.url)
            .map_err(|e| ServerError::ServerError(format!("Invalid server URL: {}", e)))?;

        let mut path = url
            .path_segments_mut()
            .map_err(|_| ServerError::ServerError("Server URL cannot be used as base URL".to_string()))?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
        drop(path);

        Ok(url)
    }

    pub fn send_event(&self, payload: &EventPayload) -> Result<EventResponse, ServerError> {
        let url = format!("{}/events", self.config.url);

        let mut request = self.client.post(&url).json(payload);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
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

    pub fn get_logs(&self, filter: &AuditFilter) -> Result<Vec<CombinedEvent>, ServerError> {
        let url = format!("{}/logs", self.config.url);

        let effective_user_login = filter
            .user_login
            .as_ref()
            .or(filter.developer_login.as_ref());
        let effective_event_type = filter
            .event_type
            .as_ref()
            .or(filter.action.as_ref());
        let effective_repo_full_name = filter
            .repo_full_name
            .as_ref()
            .or(filter.repo_name.as_ref());

        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(source) = &filter.source {
            query_params.push(("source".to_string(), source.clone()));
        }
        if let Some(start_date) = filter.start_date {
            query_params.push(("start_date".to_string(), start_date.to_string()));
        }
        if let Some(end_date) = filter.end_date {
            query_params.push(("end_date".to_string(), end_date.to_string()));
        }
        if let Some(user_login) = effective_user_login {
            query_params.push(("user_login".to_string(), user_login.clone()));
        }
        if let Some(event_type) = effective_event_type {
            query_params.push(("event_type".to_string(), event_type.clone()));
        }
        if let Some(status) = &filter.status {
            query_params.push(("status".to_string(), status.clone()));
        }
        if let Some(branch) = &filter.branch {
            query_params.push(("branch".to_string(), branch.clone()));
        }
        if let Some(repo_full_name) = effective_repo_full_name {
            query_params.push(("repo_full_name".to_string(), repo_full_name.clone()));
        }
        if let Some(org_name) = &filter.org_name {
            query_params.push(("org_name".to_string(), org_name.clone()));
        }
        query_params.push(("limit".to_string(), filter.limit.to_string()));
        query_params.push(("offset".to_string(), filter.offset.to_string()));

        let mut request = self.client.get(&url).query(&query_params);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
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
            events: Vec<CombinedEvent>,
        }

        let result: LogsResponse = response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;

        Ok(result.events)
    }

    pub fn get_stats(&self) -> Result<ServerStats, ServerError> {
        let url = format!("{}/stats", self.config.url);

        let mut request = self.client.get(&url);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
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
        let url = self.endpoint_url(&["policy", repo_name])?;

        let mut request = self.client.get(url);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
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
