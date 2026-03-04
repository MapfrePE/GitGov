use crate::models::{AuditAction, AuditLogEntry, AuditStatus, GitGovConfig};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};
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
    #[serde(default)]
    pub pipeline: PipelineHealthStats,
    pub active_devs_week: i64,
    pub active_repos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyActivityPoint {
    pub day: String,
    pub commits: i64,
    pub pushes: i64,
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
    pub desktop_pushes_today: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineHealthStats {
    pub total_7d: i64,
    pub success_7d: i64,
    pub failure_7d: i64,
    pub aborted_7d: i64,
    pub unstable_7d: i64,
    pub avg_duration_ms_7d: i64,
    pub repos_with_failures_7d: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResponse {
    pub version: String,
    pub checksum: String,
    pub config: GitGovConfig,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct JenkinsCorrelationFilter {
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub branch: Option<String>,
    pub user_login: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DailyActivityFilter {
    pub days: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PrMergeEvidenceFilter {
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub merged_by: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrMergeEvidenceEntry {
    pub id: String,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub org_name: Option<String>,
    #[serde(default)]
    pub repo_id: Option<String>,
    #[serde(default)]
    pub repo_full_name: Option<String>,
    pub delivery_id: String,
    pub pr_number: i32,
    #[serde(default)]
    pub pr_title: Option<String>,
    #[serde(default)]
    pub author_login: Option<String>,
    #[serde(default)]
    pub merged_by_login: Option<String>,
    #[serde(default)]
    pub approvers: Vec<String>,
    pub approvals_count: i32,
    #[serde(default)]
    pub head_sha: Option<String>,
    #[serde(default)]
    pub base_branch: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPipelineRun {
    pub pipeline_event_id: String,
    pub pipeline_id: String,
    pub job_name: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub triggered_by: Option<String>,
    pub ingested_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPipelineCorrelation {
    pub commit_event_id: String,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub commit_created_at: i64,
    pub user_login: String,
    pub branch: Option<String>,
    pub repo_name: Option<String>,
    pub pipeline: Option<CommitPipelineRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TicketCoverageQuery {
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub branch: Option<String>,
    pub hours: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JiraCorrelateRequest {
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub hours: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JiraCorrelateResponse {
    pub scanned_commits: i64,
    pub correlations_created: i64,
    #[serde(default)]
    pub correlated_tickets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TicketCoverageResponse {
    pub org: String,
    pub period: String,
    pub total_commits: i64,
    pub commits_with_ticket: i64,
    pub coverage_percentage: f64,
    #[serde(default)]
    pub commits_without_ticket: Vec<serde_json::Value>,
    #[serde(default)]
    pub tickets_without_commits: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectTicket {
    pub id: String,
    pub org_id: Option<String>,
    pub ticket_id: String,
    pub ticket_url: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub assignee: Option<String>,
    pub reporter: Option<String>,
    pub priority: Option<String>,
    pub ticket_type: Option<String>,
    #[serde(default)]
    pub related_commits: Vec<String>,
    #[serde(default)]
    pub related_prs: Vec<String>,
    #[serde(default)]
    pub related_branches: Vec<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub ingested_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JiraTicketDetailResponse {
    pub found: bool,
    pub ticket: Option<ProjectTicket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeyInfo {
    pub id: String,
    pub client_id: String,
    pub role: String,
    #[serde(default)]
    pub org_id: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub last_used: Option<i64>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    pub client_id: String,
    pub role: String,
    #[serde(default)]
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeApiKeyResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamRepoSummary {
    pub repo_name: String,
    pub events: i64,
    pub commits: i64,
    pub pushes: i64,
    pub blocked_pushes: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamDeveloperOverview {
    pub login: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    pub role: String,
    pub status: String,
    #[serde(default)]
    pub last_seen: Option<i64>,
    pub total_events: i64,
    pub commits: i64,
    pub pushes: i64,
    pub blocked_pushes: i64,
    pub repos_active_count: i64,
    #[serde(default)]
    pub repos: Vec<TeamRepoSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamOverviewResponse {
    pub entries: Vec<TeamDeveloperOverview>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamRepoOverview {
    pub repo_name: String,
    pub developers_active: i64,
    pub total_events: i64,
    pub commits: i64,
    pub pushes: i64,
    pub blocked_pushes: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamReposResponse {
    pub entries: Vec<TeamRepoOverview>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgUser {
    pub id: String,
    pub org_id: String,
    pub login: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    pub role: String,
    pub status: String,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub updated_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateOrgRequest {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateOrgResponse {
    pub org_id: String,
    pub login: String,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateOrgUserRequest {
    pub login: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub org_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateOrgUserResponse {
    pub user: OrgUser,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgUsersResponse {
    pub entries: Vec<OrgUser>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateOrgUserStatusRequest {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeyResponse {
    #[serde(default)]
    pub api_key: Option<String>,
    pub client_id: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgInvitation {
    pub id: String,
    pub org_id: String,
    #[serde(default)]
    pub invite_email: Option<String>,
    #[serde(default)]
    pub invite_login: Option<String>,
    pub role: String,
    pub status: String,
    pub invited_by: String,
    #[serde(default)]
    pub accepted_by: Option<String>,
    #[serde(default)]
    pub accepted_at: Option<i64>,
    #[serde(default)]
    pub revoked_by: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<i64>,
    pub expires_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateOrgInvitationRequest {
    #[serde(default)]
    pub org_name: Option<String>,
    #[serde(default)]
    pub invite_email: Option<String>,
    #[serde(default)]
    pub invite_login: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateOrgInvitationResponse {
    pub invitation: OrgInvitation,
    pub invite_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgInvitationsResponse {
    pub entries: Vec<OrgInvitation>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResendOrgInvitationRequest {
    #[serde(default)]
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AcceptOrgInvitationRequest {
    pub token: String,
    #[serde(default)]
    pub login: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AcceptOrgInvitationResponse {
    pub invitation: OrgInvitation,
    pub client_id: String,
    pub role: String,
    pub org_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExportResponse {
    pub id: String,
    pub export_type: String,
    pub record_count: i32,
    pub content_hash: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExportLogEntry {
    pub id: String,
    #[serde(default)]
    pub org_id: Option<String>,
    pub exported_by: String,
    pub export_type: String,
    #[serde(default)]
    pub date_range_start: Option<i64>,
    #[serde(default)]
    pub date_range_end: Option<i64>,
    pub filters: serde_json::Value,
    pub record_count: i32,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    pub created_at: i64,
}

// ============================================================================
// CHAT STRUCTS (must mirror server models.rs ChatAskRequest / ChatAskResponse)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatAskRequest {
    pub question: String,
    #[serde(default)]
    pub org_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatAskResponse {
    pub status: String,
    pub answer: String,
    #[serde(default)]
    pub missing_capability: Option<String>,
    pub can_report_feature: bool,
    #[serde(default)]
    pub data_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureRequestInput {
    pub question: String,
    #[serde(default)]
    pub missing_capability: Option<String>,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub user_login: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureRequestCreated {
    pub id: String,
    pub status: String,
}

pub struct ControlPlaneClient {
    config: ServerConfig,
    client: reqwest::blocking::Client,
}

fn shared_http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .expect("failed to build shared control plane HTTP client")
    })
}

fn normalize_loopback_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let Ok(mut parsed) = reqwest::Url::parse(trimmed) else {
        return trimmed.to_string();
    };

    if parsed.host_str() == Some("localhost")
        && parsed.set_host(Some("127.0.0.1")).is_ok() {
            return parsed.to_string();
        }

    trimmed.to_string()
}

impl ControlPlaneClient {
    pub fn new(mut config: ServerConfig) -> Self {
        config.url = normalize_loopback_url(&config.url);
        Self {
            config,
            client: shared_http_client().clone(),
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

    pub fn get_daily_activity(
        &self,
        filter: &DailyActivityFilter,
    ) -> Result<Vec<DailyActivityPoint>, ServerError> {
        let url = format!("{}/stats/daily", self.config.url);
        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(days) = filter.days {
            query_params.push(("days".to_string(), days.to_string()));
        }

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

        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn get_team_overview(
        &self,
        org_name: Option<&str>,
        status: Option<&str>,
        days: i64,
        limit: usize,
        offset: usize,
    ) -> Result<TeamOverviewResponse, ServerError> {
        let url = format!("{}/team/overview", self.config.url);
        let mut query_params: Vec<(String, String)> = vec![
            ("days".to_string(), days.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("offset".to_string(), offset.to_string()),
        ];
        if let Some(org_name) = org_name {
            query_params.push(("org_name".to_string(), org_name.to_string()));
        }
        if let Some(status) = status {
            query_params.push(("status".to_string(), status.to_string()));
        }

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
        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn get_team_repos(
        &self,
        org_name: Option<&str>,
        days: i64,
        limit: usize,
        offset: usize,
    ) -> Result<TeamReposResponse, ServerError> {
        let url = format!("{}/team/repos", self.config.url);
        let mut query_params: Vec<(String, String)> = vec![
            ("days".to_string(), days.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("offset".to_string(), offset.to_string()),
        ];
        if let Some(org_name) = org_name {
            query_params.push(("org_name".to_string(), org_name.to_string()));
        }

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

    pub fn get_jenkins_correlations(
        &self,
        filter: &JenkinsCorrelationFilter,
    ) -> Result<Vec<CommitPipelineCorrelation>, ServerError> {
        let url = format!("{}/integrations/jenkins/correlations", self.config.url);

        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(org_name) = &filter.org_name {
            query_params.push(("org_name".to_string(), org_name.clone()));
        }
        if let Some(repo_full_name) = &filter.repo_full_name {
            query_params.push(("repo_full_name".to_string(), repo_full_name.clone()));
        }
        if let Some(branch) = &filter.branch {
            query_params.push(("branch".to_string(), branch.clone()));
        }
        if let Some(user_login) = &filter.user_login {
            query_params.push(("user_login".to_string(), user_login.clone()));
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
        struct CorrelationsResponse {
            correlations: Vec<CommitPipelineCorrelation>,
        }

        let result: CorrelationsResponse = response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;

        Ok(result.correlations)
    }

    pub fn get_pr_merges(
        &self,
        filter: &PrMergeEvidenceFilter,
    ) -> Result<Vec<PrMergeEvidenceEntry>, ServerError> {
        let url = format!("{}/pr-merges", self.config.url);

        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(org_name) = &filter.org_name {
            query_params.push(("org_name".to_string(), org_name.clone()));
        }
        if let Some(repo_full_name) = &filter.repo_full_name {
            query_params.push(("repo_full_name".to_string(), repo_full_name.clone()));
        }
        if let Some(merged_by) = &filter.merged_by {
            query_params.push(("merged_by".to_string(), merged_by.clone()));
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
        struct PrMergesResponse {
            entries: Vec<PrMergeEvidenceEntry>,
        }

        let result: PrMergesResponse = response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;

        Ok(result.entries)
    }

    pub fn get_jira_ticket_coverage(
        &self,
        query: &TicketCoverageQuery,
    ) -> Result<TicketCoverageResponse, ServerError> {
        let url = format!("{}/integrations/jira/ticket-coverage", self.config.url);

        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(org_name) = &query.org_name {
            query_params.push(("org_name".to_string(), org_name.clone()));
        }
        if let Some(repo_full_name) = &query.repo_full_name {
            query_params.push(("repo_full_name".to_string(), repo_full_name.clone()));
        }
        if let Some(branch) = &query.branch {
            query_params.push(("branch".to_string(), branch.clone()));
        }
        if let Some(hours) = query.hours {
            query_params.push(("hours".to_string(), hours.to_string()));
        }

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

        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn correlate_jira_tickets(
        &self,
        request_body: &JiraCorrelateRequest,
    ) -> Result<JiraCorrelateResponse, ServerError> {
        let url = format!("{}/integrations/jira/correlate", self.config.url);

        let mut request = self.client.post(&url).json(request_body);
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

    pub fn get_jira_ticket_detail(
        &self,
        ticket_id: &str,
    ) -> Result<JiraTicketDetailResponse, ServerError> {
        let url = self.endpoint_url(&["integrations", "jira", "tickets", ticket_id])?;
        let mut request = self.client.get(url);
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        let response = request
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        if response.status().as_u16() == 404 {
            return Ok(JiraTicketDetailResponse { found: false, ticket: None });
        }
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

    pub fn export_events(
        &self,
        export_type: &str,
        start_date: Option<i64>,
        end_date: Option<i64>,
        org_name: Option<&str>,
    ) -> Result<ExportResponse, ServerError> {
        let url = format!("{}/export", self.config.url);
        let body = serde_json::json!({
            "export_type": export_type,
            "start_date": start_date,
            "end_date": end_date,
            "org_name": org_name,
        });
        let mut request = self.client.post(&url).json(&body);
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

    pub fn list_exports(&self) -> Result<Vec<ExportLogEntry>, ServerError> {
        let url = format!("{}/exports", self.config.url);
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

    pub fn get_me(&self) -> Result<MeResponse, ServerError> {
        let url = format!("{}/me", self.config.url);
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

    pub fn create_org(&self, payload: &CreateOrgRequest) -> Result<CreateOrgResponse, ServerError> {
        let url = format!("{}/orgs", self.config.url);
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

    pub fn create_org_user(
        &self,
        payload: &CreateOrgUserRequest,
    ) -> Result<CreateOrgUserResponse, ServerError> {
        let url = format!("{}/org-users", self.config.url);
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

    pub fn list_org_users(
        &self,
        org_name: Option<&str>,
        status: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<OrgUsersResponse, ServerError> {
        let url = format!("{}/org-users", self.config.url);
        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(org_name) = org_name {
            query_params.push(("org_name".to_string(), org_name.to_string()));
        }
        if let Some(status) = status {
            query_params.push(("status".to_string(), status.to_string()));
        }
        query_params.push(("limit".to_string(), limit.to_string()));
        query_params.push(("offset".to_string(), offset.to_string()));

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
        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn update_org_user_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<OrgUser, ServerError> {
        let url = self.endpoint_url(&["org-users", user_id, "status"])?;
        let mut request = self
            .client
            .patch(url)
            .json(&UpdateOrgUserStatusRequest {
                status: status.to_string(),
            });
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

    pub fn create_api_key_for_org_user(&self, user_id: &str) -> Result<ApiKeyResponse, ServerError> {
        let url = self.endpoint_url(&["org-users", user_id, "api-key"])?;
        let mut request = self.client.post(url).body("");
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

    pub fn create_org_invitation(
        &self,
        payload: &CreateOrgInvitationRequest,
    ) -> Result<CreateOrgInvitationResponse, ServerError> {
        let url = format!("{}/org-invitations", self.config.url);
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

    pub fn list_org_invitations(
        &self,
        org_name: Option<&str>,
        status: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<OrgInvitationsResponse, ServerError> {
        let url = format!("{}/org-invitations", self.config.url);
        let mut query_params: Vec<(String, String)> = Vec::new();
        if let Some(org_name) = org_name {
            query_params.push(("org_name".to_string(), org_name.to_string()));
        }
        if let Some(status) = status {
            query_params.push(("status".to_string(), status.to_string()));
        }
        query_params.push(("limit".to_string(), limit.to_string()));
        query_params.push(("offset".to_string(), offset.to_string()));

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
        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    pub fn resend_org_invitation(
        &self,
        invitation_id: &str,
        payload: &ResendOrgInvitationRequest,
    ) -> Result<CreateOrgInvitationResponse, ServerError> {
        let url = self.endpoint_url(&["org-invitations", invitation_id, "resend"])?;
        let mut request = self.client.post(url).json(payload);
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

    pub fn revoke_org_invitation(&self, invitation_id: &str) -> Result<OrgInvitation, ServerError> {
        let url = self.endpoint_url(&["org-invitations", invitation_id, "revoke"])?;
        let mut request = self.client.post(url).body("");
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

    pub fn preview_org_invitation(&self, token: &str) -> Result<OrgInvitation, ServerError> {
        let url = self.endpoint_url(&["org-invitations", "preview", token])?;
        let response = self
            .client
            .get(url)
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

    pub fn accept_org_invitation(
        &self,
        payload: &AcceptOrgInvitationRequest,
    ) -> Result<AcceptOrgInvitationResponse, ServerError> {
        let url = format!("{}/org-invitations/accept", self.config.url);
        let response = self
            .client
            .post(url)
            .json(payload)
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

    pub fn list_api_keys(&self) -> Result<Vec<ApiKeyInfo>, ServerError> {
        let url = format!("{}/api-keys", self.config.url);
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

    pub fn revoke_api_key(&self, key_id: &str) -> Result<RevokeApiKeyResponse, ServerError> {
        let url = self.endpoint_url(&["api-keys", key_id, "revoke"])?;
        let mut request = self.client.post(url).body("");
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        let response = request
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;
        if !response.status().is_success() && response.status().as_u16() != 404 {
            return Err(ServerError::ServerError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }
        response
            .json()
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    // ── Chat & Feature Requests ─────────────────────────────────────────────

    pub fn chat_ask(&self, request: &ChatAskRequest) -> Result<ChatAskResponse, ServerError> {
        let url = format!("{}/chat/ask", self.config.url);
        let mut req = self.client.post(&url).json(request);
        if let Some(ref api_key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        let response = req
            .send()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|e| ServerError::NetworkError(e.to_string()))?;

        if let Ok(parsed) = serde_json::from_str::<ChatAskResponse>(&body) {
            return Ok(parsed);
        }

        if !status.is_success() {
            let snippet = body.chars().take(180).collect::<String>();
            return Err(ServerError::ServerError(format!(
                "Server returned status: {} ({})",
                status, snippet
            )));
        }

        Err(ServerError::SerializationError(
            "Invalid chat response payload".to_string(),
        ))
    }

    pub fn create_feature_request(
        &self,
        input: &FeatureRequestInput,
    ) -> Result<FeatureRequestCreated, ServerError> {
        let url = format!("{}/feature-requests", self.config.url);
        let mut req = self.client.post(&url).json(input);
        if let Some(ref api_key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        let response = req
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
            action: entry.action,
            branch: entry.branch.clone(),
            files: entry.files.clone(),
            commit_hash: entry.commit_hash.clone(),
            status: entry.status,
            reason: entry.reason.clone(),
            repo_name,
            repo_owner,
        }
    }
}
