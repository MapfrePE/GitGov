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
    pub repo_full_name: Option<String>,
    pub org_name: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditStats {
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
    pub by_type: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientEventStats {
    pub total: i64,
    pub today: i64,
    pub blocked_today: i64,
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
