use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// ORGANIZATIONS & REPOS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    pub id: String,
    pub github_id: Option<i64>,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: String,
    pub org_id: Option<String>,
    pub github_id: Option<i64>,
    pub full_name: String,
    pub name: String,
    pub private: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub org_id: String,
    pub github_login: String,
    pub github_id: Option<i64>,
    pub role: UserRole,
    pub groups: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Admin,
    Architect,
    Developer,
    PM,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "Admin",
            UserRole::Architect => "Architect",
            UserRole::Developer => "Developer",
            UserRole::PM => "PM",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Admin" => UserRole::Admin,
            "Architect" => UserRole::Architect,
            "PM" => UserRole::PM,
            _ => UserRole::Developer,
        }
    }
}

// ============================================================================
// GITHUB EVENTS (Source of Truth - from webhooks)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEvent {
    pub id: String,
    pub org_id: Option<String>,
    pub repo_id: Option<String>,
    pub delivery_id: String,
    pub event_type: String,
    pub actor_login: Option<String>,
    pub actor_id: Option<i64>,
    pub ref_name: Option<String>,
    pub ref_type: Option<String>,
    pub before_sha: Option<String>,
    pub after_sha: Option<String>,
    pub commit_shas: Vec<String>,
    pub commits_count: i32,
    pub payload: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubWebhookPayload {
    pub delivery_id: String,
    pub event_type: String,
    pub signature: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEvent {
    pub r#ref: String,
    pub before: String,
    pub after: String,
    pub repository: GitHubRepository,
    pub sender: GitHubUser,
    pub commits: Vec<GitHubCommit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEvent {
    pub r#ref: String,
    pub ref_type: String,
    pub repository: GitHubRepository,
    pub sender: GitHubUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRepository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub owner: GitHubUser,
    pub private: bool,
    pub organization: Option<GitHubOrganization>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubOrganization {
    pub login: String,
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommit {
    pub id: String,
    pub message: String,
    pub author: GitHubCommitAuthor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommitAuthor {
    pub name: String,
    pub email: String,
}

// ============================================================================
// CLIENT EVENTS (Telemetry from Desktop App)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEvent {
    pub id: String,
    pub org_id: Option<String>,
    pub repo_id: Option<String>,
    pub event_uuid: String,
    pub event_type: ClientEventType,
    pub user_login: String,
    pub user_name: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub files: Vec<String>,
    pub status: EventStatus,
    pub reason: Option<String>,
    pub metadata: serde_json::Value,
    pub client_version: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClientEventType {
    AttemptPush,
    BlockedPush,
    SuccessfulPush,
    CreateBranch,
    BlockedBranch,
    StageFiles,
    Commit,
    CheckoutBranch,
    Login,
    Logout,
}

impl ClientEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientEventType::AttemptPush => "attempt_push",
            ClientEventType::BlockedPush => "blocked_push",
            ClientEventType::SuccessfulPush => "successful_push",
            ClientEventType::CreateBranch => "create_branch",
            ClientEventType::BlockedBranch => "blocked_branch",
            ClientEventType::StageFiles => "stage_files",
            ClientEventType::Commit => "commit",
            ClientEventType::CheckoutBranch => "checkout_branch",
            ClientEventType::Login => "login",
            ClientEventType::Logout => "logout",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "attempt_push" => ClientEventType::AttemptPush,
            "blocked_push" => ClientEventType::BlockedPush,
            "successful_push" => ClientEventType::SuccessfulPush,
            "create_branch" => ClientEventType::CreateBranch,
            "blocked_branch" => ClientEventType::BlockedBranch,
            "stage_files" => ClientEventType::StageFiles,
            "commit" => ClientEventType::Commit,
            "checkout_branch" => ClientEventType::CheckoutBranch,
            "login" => ClientEventType::Login,
            "logout" => ClientEventType::Logout,
            _ => ClientEventType::AttemptPush,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    Success,
    Blocked,
    Failed,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventStatus::Success => "success",
            EventStatus::Blocked => "blocked",
            EventStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "success" => EventStatus::Success,
            "blocked" => EventStatus::Blocked,
            _ => EventStatus::Failed,
        }
    }
}

// ============================================================================
// BATCH INGEST FROM CLIENT
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEventBatch {
    pub events: Vec<ClientEventInput>,
    pub client_id: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEventInput {
    pub event_uuid: String,
    pub event_type: String,
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub user_login: String,
    pub user_name: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub files: Vec<String>,
    pub status: String,
    pub reason: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEventResponse {
    pub accepted: Vec<String>,
    pub duplicates: Vec<String>,
    pub errors: Vec<EventError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventError {
    pub event_uuid: String,
    pub error: String,
}

// ============================================================================
// VIOLATIONS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub id: String,
    pub org_id: Option<String>,
    pub repo_id: Option<String>,
    pub github_event_id: Option<String>,
    pub client_event_id: Option<String>,
    pub violation_type: ViolationType,
    pub severity: Severity,
    pub user_login: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub details: serde_json::Value,
    pub resolved: bool,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    UnauthorizedPush,
    BranchProtection,
    NamingViolation,
    PathViolation,
    CommitMessageViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

// ============================================================================
// FILTERS & STATS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventFilter {
    pub source: Option<String>,
    pub event_type: Option<String>,
    pub user_login: Option<String>,
    pub branch: Option<String>,
    pub repo_full_name: Option<String>,
    pub org_name: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditStats {
    pub github_events: GitHubEventStats,
    pub client_events: ClientEventStats,
    pub violations: ViolationStats,
    #[serde(default)]
    pub pipeline: PipelineHealthStats,
    pub active_devs_week: i64,
    pub active_repos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubEventStats {
    pub total: i64,
    pub today: i64,
    pub pushes_today: i64,
    #[serde(default)]
    pub by_type: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientEventStats {
    pub total: i64,
    pub today: i64,
    pub blocked_today: i64,
    #[serde(default)]
    pub desktop_pushes_today: i64,
    #[serde(default)]
    pub by_type: HashMap<String, i64>,
    #[serde(default)]
    pub by_status: HashMap<String, i64>,
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
pub struct CombinedEvent {
    pub id: String,
    pub source: String,
    pub event_type: String,
    pub created_at: i64,
    pub user_login: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub details: serde_json::Value,
}

// ============================================================================
// POLICIES (gitgov.toml)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitGovConfig {
    #[serde(default)]
    pub branches: BranchConfig,
    #[serde(default)]
    pub groups: HashMap<String, GroupConfig>,
    #[serde(default)]
    pub admins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BranchConfig {
    pub patterns: Vec<String>,
    pub protected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupConfig {
    pub members: Vec<String>,
    pub allowed_branches: Vec<String>,
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResponse {
    pub version: String,
    pub checksum: String,
    pub config: GitGovConfig,
    pub updated_at: i64,
}

// ============================================================================
// LEGACY SUPPORT (keep for backward compatibility)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
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
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum AuditAction {
    Push,
    BranchCreate,
    StageFile,
    Commit,
    BlockedPush,
    BlockedBranch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum AuditStatus {
    Success,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditFilter {
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub developer_login: Option<String>,
    pub action: Option<String>,
    pub status: Option<String>,
    pub branch: Option<String>,
    pub repo_name: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

// ============================================================================
// API KEYS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: UserRole,
    pub exp: usize,
}

// ============================================================================
// NONCOMPLIANCE SIGNALS (NO binario - confidence levels)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoncomplianceSignal {
    pub id: String,
    pub org_id: Option<String>,
    pub repo_id: Option<String>,
    pub github_event_id: Option<String>,
    pub client_event_id: Option<String>,
    pub signal_type: String,
    pub confidence: String,
    pub actor_login: String,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub evidence: serde_json::Value,
    pub context: serde_json::Value,
    pub status: String,
    pub investigated_by: Option<String>,
    pub investigated_at: Option<i64>,
    pub investigation_notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    UntrustedPath,
    MissingTelemetry,
    PolicyViolation,
    CorrelationMismatch,
}

impl SignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalType::UntrustedPath => "untrusted_path",
            SignalType::MissingTelemetry => "missing_telemetry",
            SignalType::PolicyViolation => "policy_violation",
            SignalType::CorrelationMismatch => "correlation_mismatch",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "untrusted_path" => SignalType::UntrustedPath,
            "missing_telemetry" => SignalType::MissingTelemetry,
            "policy_violation" => SignalType::PolicyViolation,
            _ => SignalType::CorrelationMismatch,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

impl ConfidenceLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfidenceLevel::High => "high",
            ConfidenceLevel::Medium => "medium",
            ConfidenceLevel::Low => "low",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "high" => ConfidenceLevel::High,
            "medium" => ConfidenceLevel::Medium,
            _ => ConfidenceLevel::Low,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SignalStatus {
    Pending,
    Investigating,
    Confirmed,
    Dismissed,
}

impl SignalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalStatus::Pending => "pending",
            SignalStatus::Investigating => "investigating",
            SignalStatus::Confirmed => "confirmed",
            SignalStatus::Dismissed => "dismissed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "investigating" => SignalStatus::Investigating,
            "confirmed" => SignalStatus::Confirmed,
            "dismissed" => SignalStatus::Dismissed,
            _ => SignalStatus::Pending,
        }
    }
}

// ============================================================================
// POLICY HISTORY
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyHistory {
    pub id: String,
    pub repo_id: String,
    pub config: GitGovConfig,
    pub checksum: String,
    pub changed_by: String,
    pub change_type: String,
    pub previous_checksum: Option<String>,
    pub created_at: i64,
}

// ============================================================================
// CORRELATION CONFIG
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationConfig {
    pub org_id: Option<String>,
    pub correlation_window_minutes: i32,
    pub bypass_tolerance_minutes: i32,
    pub clock_skew_seconds: i32,
    pub auto_create_violations: bool,
}

impl Default for CorrelationConfig {
    fn default() -> Self {
        Self {
            org_id: None,
            correlation_window_minutes: 15,
            bypass_tolerance_minutes: 30,
            clock_skew_seconds: 60,
            auto_create_violations: false,
        }
    }
}

// ============================================================================
// EXPORT LOGS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLog {
    pub id: String,
    pub org_id: Option<String>,
    pub exported_by: String,
    pub export_type: String,
    pub date_range_start: Option<i64>,
    pub date_range_end: Option<i64>,
    pub filters: serde_json::Value,
    pub record_count: i32,
    pub content_hash: Option<String>,
    pub file_path: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    pub export_type: String,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub filters: Option<serde_json::Value>,
    pub org_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResponse {
    pub id: String,
    pub export_type: String,
    pub record_count: i32,
    pub content_hash: String,
    pub data: Option<serde_json::Value>,
    pub created_at: i64,
}

// ============================================================================
// COMPLIANCE DASHBOARD
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComplianceDashboard {
    pub signals: SignalStats,
    pub correlation: CorrelationStats,
    pub policy: PolicyStats,
    pub exports: ExportStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignalStats {
    pub total: i64,
    pub pending: i64,
    pub high_confidence: i64,
    pub by_type: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorrelationStats {
    pub github_pushes_24h: i64,
    pub client_pushes_24h: i64,
    pub correlation_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyStats {
    pub repos_with_policy: i64,
    pub total_repos: i64,
    pub recent_changes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExportStats {
    pub total: i64,
    pub last_7_days: i64,
}

// ============================================================================
// SERVER HEALTH
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedHealthResponse {
    pub status: String,
    pub version: String,
    pub database: DatabaseHealth,
    pub uptime_seconds: i64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub connected: bool,
    pub latency_ms: Option<i64>,
    pub pending_events: Option<i64>,
}

// ============================================================================
// GOVERNANCE EVENTS (Audit Log Streaming)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEvent {
    pub id: String,
    pub org_id: Option<String>,
    pub repo_id: Option<String>,
    pub delivery_id: String,
    pub event_type: String,
    pub actor_login: Option<String>,
    pub target: Option<String>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub payload: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAuditLogEntry {
    #[serde(rename = "@timestamp")]
    pub timestamp: i64,
    pub action: String,
    pub actor: Option<String>,
    pub actor_location: Option<GitHubAuditActorLocation>,
    pub org: Option<String>,
    pub repo: Option<String>,
    pub repository: Option<String>,
    pub repository_id: Option<i64>,
    pub user: Option<String>,
    pub team: Option<String>,
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAuditActorLocation {
    pub country_code: Option<String>,
    pub country_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStreamBatch {
    pub entries: Vec<GitHubAuditLogEntry>,
    pub org_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStreamResponse {
    pub accepted: i32,
    pub filtered: i32,
    pub errors: Vec<String>,
}

// ============================================================================
// PIPELINE EVENTS (V1.2-A Jenkins Integration)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEvent {
    pub id: String,
    pub org_id: Option<String>,
    pub pipeline_id: String,
    pub job_name: String,
    pub status: PipelineStatus,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub repo_full_name: Option<String>,
    pub duration_ms: Option<i64>,
    pub triggered_by: Option<String>,
    #[serde(default)]
    pub stages: Vec<PipelineStage>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub ingested_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStatus {
    Success,
    Failure,
    Aborted,
    Unstable,
}

impl PipelineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineStatus::Success => "success",
            PipelineStatus::Failure => "failure",
            PipelineStatus::Aborted => "aborted",
            PipelineStatus::Unstable => "unstable",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(PipelineStatus::Success),
            "failure" => Some(PipelineStatus::Failure),
            "aborted" => Some(PipelineStatus::Aborted),
            "unstable" => Some(PipelineStatus::Unstable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JenkinsPipelineEventInput {
    pub pipeline_id: String,
    pub job_name: String,
    pub status: String,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub repo_full_name: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub triggered_by: Option<String>,
    #[serde(default)]
    pub stages: Vec<PipelineStage>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JenkinsPipelineEventResponse {
    pub accepted: bool,
    pub duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JenkinsIntegrationStatusResponse {
    pub ok: bool,
    #[serde(default)]
    pub last_ingest_at: Option<i64>,
    #[serde(default)]
    pub recent_events_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JenkinsCorrelationFilter {
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub branch: Option<String>,
    pub user_login: Option<String>,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JenkinsCorrelationsResponse {
    pub correlations: Vec<CommitPipelineCorrelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheckRequest {
    pub repo: String,
    #[serde(default)]
    pub commit: Option<String>,
    pub branch: String,
    #[serde(default)]
    pub user_login: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyCheckResponse {
    pub advisory: bool,
    pub allowed: bool,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub evaluated_rules: Vec<String>,
}

// ============================================================================
// JIRA / TICKET COVERAGE (V1.2-B groundwork)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitTicketCorrelation {
    pub id: String,
    pub org_id: Option<String>,
    pub commit_sha: String,
    pub ticket_id: String,
    pub correlation_source: String,
    pub confidence: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraWebhookEvent {
    #[serde(default)]
    pub webhook_event: Option<String>,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub issue: Option<serde_json::Value>,
    #[serde(default)]
    pub user: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraWebhookIngestResponse {
    pub accepted: bool,
    pub duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JiraIntegrationStatusResponse {
    pub ok: bool,
    #[serde(default)]
    pub last_ingest_at: Option<i64>,
    #[serde(default)]
    pub recent_tickets_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JiraTicketDetailResponse {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<ProjectTicket>,
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
pub struct JiraCorrelateRequest {
    #[serde(default)]
    pub org_name: Option<String>,
    #[serde(default)]
    pub repo_full_name: Option<String>,
    #[serde(default)]
    pub hours: Option<i64>,
    #[serde(default)]
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
pub struct TicketCoverageQuery {
    #[serde(default)]
    pub org_name: Option<String>,
    #[serde(default)]
    pub repo_full_name: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub hours: Option<i64>,
}

pub const RELEVANT_AUDIT_ACTIONS: &[&str] = &[
    "protected_branch.create",
    "protected_branch.destroy",
    "protected_branch.update_name",
    "protected_branch.update_admin_enforced",
    "protected_branch.update_pull_request_reviews_enforcement_level",
    "protected_branch.update_required_pull_request_reviews",
    "protected_branch.update_required_status_checks",
    "protected_branch.update_required_approving_review_count",
    "protected_branch.update_signature_requirement_enforcement_level",
    "protected_branch.update_strict_required_status_checks_policy",
    "repository_ruleset.create",
    "repository_ruleset.destroy",
    "repository_ruleset.update",
    "repository_ruleset.clear_custom_properties",
    "repo.access",
    "repo.permissions_granted",
    "repo.permissions_revoked",
    "team.add_repository",
    "team.remove_repository",
    "team.update_repository_permission",
    "org.update_member_repository_creation_permission",
    "org.update_default_repository_permission",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_role_roundtrip() {
        let roles = [UserRole::Admin, UserRole::Architect, UserRole::Developer, UserRole::PM];
        for role in &roles {
            assert_eq!(&UserRole::from_str(role.as_str()), role);
        }
    }

    #[test]
    fn user_role_unknown_defaults_to_developer() {
        assert_eq!(UserRole::from_str("unknown"), UserRole::Developer);
        assert_eq!(UserRole::from_str(""), UserRole::Developer);
    }

    #[test]
    fn client_event_type_roundtrip() {
        let types = [
            ClientEventType::AttemptPush,
            ClientEventType::BlockedPush,
            ClientEventType::SuccessfulPush,
            ClientEventType::CreateBranch,
            ClientEventType::BlockedBranch,
            ClientEventType::StageFiles,
            ClientEventType::Commit,
            ClientEventType::CheckoutBranch,
            ClientEventType::Login,
            ClientEventType::Logout,
        ];
        for t in &types {
            assert_eq!(&ClientEventType::from_str(t.as_str()), t);
        }
    }

    #[test]
    fn event_status_roundtrip() {
        assert_eq!(EventStatus::from_str("success"), EventStatus::Success);
        assert_eq!(EventStatus::from_str("blocked"), EventStatus::Blocked);
        assert_eq!(EventStatus::from_str("failed"), EventStatus::Failed);
        assert_eq!(EventStatus::from_str("unknown"), EventStatus::Failed);
    }

    #[test]
    fn pipeline_status_roundtrip() {
        assert_eq!(PipelineStatus::from_str("success"), Some(PipelineStatus::Success));
        assert_eq!(PipelineStatus::from_str("failure"), Some(PipelineStatus::Failure));
        assert_eq!(PipelineStatus::from_str("aborted"), Some(PipelineStatus::Aborted));
        assert_eq!(PipelineStatus::from_str("unstable"), Some(PipelineStatus::Unstable));
        assert_eq!(PipelineStatus::from_str("invalid"), None);
    }

    #[test]
    fn signal_type_roundtrip() {
        let types = [
            SignalType::UntrustedPath,
            SignalType::MissingTelemetry,
            SignalType::PolicyViolation,
            SignalType::CorrelationMismatch,
        ];
        for t in &types {
            assert_eq!(&SignalType::from_str(t.as_str()), t);
        }
    }

    #[test]
    fn confidence_level_roundtrip() {
        assert_eq!(ConfidenceLevel::from_str("high"), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_str("medium"), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_str("low"), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_str("unknown"), ConfidenceLevel::Low);
    }

    #[test]
    fn signal_status_roundtrip() {
        assert_eq!(SignalStatus::from_str("pending"), SignalStatus::Pending);
        assert_eq!(SignalStatus::from_str("investigating"), SignalStatus::Investigating);
        assert_eq!(SignalStatus::from_str("confirmed"), SignalStatus::Confirmed);
        assert_eq!(SignalStatus::from_str("dismissed"), SignalStatus::Dismissed);
        assert_eq!(SignalStatus::from_str("unknown"), SignalStatus::Pending);
    }

    #[test]
    fn client_event_batch_deserialize() {
        let json = r#"{
            "events": [{
                "event_uuid": "abc-123",
                "event_type": "commit",
                "repo_full_name": "MapfrePE/GitGov",
                "branch": "main",
                "user_login": "dev1",
                "files": ["src/main.rs"],
                "status": "success"
            }]
        }"#;
        let batch: ClientEventBatch = serde_json::from_str(json).unwrap();
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].event_type, "commit");
        assert!(batch.client_id.is_none());
    }

    #[test]
    fn policy_check_response_default() {
        let resp = PolicyCheckResponse::default();
        assert!(!resp.advisory);
        assert!(!resp.allowed);
        assert!(resp.reasons.is_empty());
        assert!(resp.warnings.is_empty());
    }

    #[test]
    fn jenkins_pipeline_input_deserialize_with_defaults() {
        let json = r#"{
            "pipeline_id": "build-123",
            "job_name": "main-build",
            "status": "success"
        }"#;
        let input: JenkinsPipelineEventInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.pipeline_id, "build-123");
        assert!(input.commit_sha.is_none());
        assert!(input.stages.is_empty());
        assert!(input.artifacts.is_empty());
    }

    // ── Golden Path contract tests ────────────────────────────────────────────
    // Validate the exact JSON shape the Desktop sends for each step of the
    // Golden Path: stage_files → commit → attempt_push → successful_push.
    // Pure deserialisation — no DB or server required; run in CI via `cargo test`.

    fn gp_batch(event_type: &str, extra_fields: &str) -> ClientEventBatch {
        let json = format!(
            r#"{{
                "events": [{{
                    "event_uuid": "00000000-0000-0000-0000-000000000001",
                    "event_type": "{event_type}",
                    "user_login": "dev1",
                    "repo_full_name": "MapfrePE/GitGov",
                    "branch": "feat/golden",
                    "files": ["src/main.rs", "src/lib.rs"],
                    "status": "success"
                    {extra_fields}
                }}],
                "client_version": "1.0.0"
            }}"#
        );
        serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to parse {event_type} batch: {e}"))
    }

    #[test]
    fn golden_path_stage_files_contract() {
        let batch = gp_batch("stage_files", "");
        assert_eq!(batch.events.len(), 1);
        let ev = &batch.events[0];
        assert_eq!(ev.event_type, "stage_files");
        assert_eq!(ev.user_login, "dev1");
        assert!(!ev.files.is_empty(), "stage_files must carry file list");
        assert_eq!(ev.status, "success");
        assert!(!ev.event_uuid.is_empty(), "event_uuid required for dedup");
    }

    #[test]
    fn golden_path_commit_contract() {
        let batch = gp_batch(
            "commit",
            r#", "commit_sha": "abc123def4567890abc123def4567890abc12345""#,
        );
        let ev = &batch.events[0];
        assert_eq!(ev.event_type, "commit");
        assert!(ev.commit_sha.is_some(), "commit event must carry commit_sha");
        assert_eq!(ev.status, "success");
    }

    #[test]
    fn golden_path_attempt_push_contract() {
        let batch = gp_batch("attempt_push", "");
        let ev = &batch.events[0];
        assert_eq!(ev.event_type, "attempt_push");
        assert_eq!(ev.branch.as_deref(), Some("feat/golden"));
        assert_eq!(ev.status, "success");
    }

    #[test]
    fn golden_path_successful_push_contract() {
        let batch = gp_batch("successful_push", "");
        let ev = &batch.events[0];
        assert_eq!(ev.event_type, "successful_push");
        assert_eq!(ev.status, "success");
        assert!(!ev.event_uuid.is_empty());
    }

    #[test]
    fn golden_path_response_accepted_shape() {
        // Validates /events response — Desktop parses this to know if accepted or duped.
        let json = r#"{"accepted":["00000000-0000-0000-0000-000000000001"],"duplicates":[],"errors":[]}"#;
        let resp: ClientEventResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.accepted.len(), 1);
        assert!(resp.duplicates.is_empty());
        assert!(resp.errors.is_empty());
    }

    #[test]
    fn golden_path_duplicate_detected_in_response() {
        // Server returns the same UUID as a duplicate on second send.
        let json = r#"{"accepted":[],"duplicates":["00000000-0000-0000-0000-000000000001"],"errors":[]}"#;
        let resp: ClientEventResponse = serde_json::from_str(json).unwrap();
        assert!(resp.accepted.is_empty());
        assert_eq!(resp.duplicates.len(), 1);
    }

    #[test]
    fn relevant_audit_actions_contains_expected() {
        assert!(RELEVANT_AUDIT_ACTIONS.contains(&"protected_branch.create"));
        assert!(RELEVANT_AUDIT_ACTIONS.contains(&"repo.access"));
        assert!(!RELEVANT_AUDIT_ACTIONS.contains(&"random_action"));
    }

    // Pagination defaults — regression tests for "missing field offset/limit"
    #[test]
    fn event_filter_offset_optional_defaults_to_zero() {
        let f: EventFilter = serde_json::from_str(r#"{"limit": 5}"#).unwrap();
        assert_eq!(f.offset, 0);
        assert_eq!(f.limit, 5);
    }

    #[test]
    fn event_filter_all_pagination_optional() {
        let f: EventFilter = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(f.offset, 0);
        assert_eq!(f.limit, 0); // 0 → handler uses its fallback default
    }

    #[test]
    fn event_filter_explicit_offset_respected() {
        let f: EventFilter = serde_json::from_str(r#"{"limit": 10, "offset": 25}"#).unwrap();
        assert_eq!(f.offset, 25);
        assert_eq!(f.limit, 10);
    }

    #[test]
    fn jenkins_correlation_filter_offset_optional() {
        let f: JenkinsCorrelationFilter = serde_json::from_str(r#"{"limit": 10}"#).unwrap();
        assert_eq!(f.offset, 0);
        assert_eq!(f.limit, 10);
    }

    #[test]
    fn jenkins_correlation_filter_all_pagination_optional() {
        let f: JenkinsCorrelationFilter = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(f.offset, 0);
        assert_eq!(f.limit, 0); // 0 → handler uses its fallback default (20)
    }
}
