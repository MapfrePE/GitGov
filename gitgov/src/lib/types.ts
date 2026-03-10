export interface FileChange {
  path: string
  status: 'Modified' | 'Added' | 'Deleted' | 'Renamed' | 'Untracked'
  staged: boolean
  diff?: string
}

export interface AuditLogEntry {
  id: string
  timestamp: number
  developer_login: string
  developer_name: string
  action: 'Push' | 'BranchCreate' | 'StageFile' | 'Commit' | 'BlockedPush' | 'BlockedBranch'
  branch: string
  files: string[]
  commit_hash?: string
  status: 'Success' | 'Blocked' | 'Failed'
  reason?: string
}

export interface EventDetails {
  commit_sha?: string
  commit_message?: string
  files?: string[]
  pipeline_id?: string
  job_name?: string
  pipeline_status?: string
  ticket_key?: string
  ticket_summary?: string
  reason?: string
  [key: string]: unknown
}

export interface CombinedEvent {
  id: string
  source: string
  event_type: string
  created_at: number
  user_login?: string
  repo_name?: string
  branch?: string
  status?: string
  details: EventDetails
}

export interface AuditFilter {
  start_date?: number
  end_date?: number
  developer_login?: string
  action?: string
  status?: string
  branch?: string
  before_created_at?: number
  before_id?: string
  limit: number
  // Legacy fallback for /logs; prefer keyset cursor when possible.
  offset: number
}

export interface AuditStats {
  pushes_today: number
  blocked_today: number
  active_devs_this_week: number
  most_frequent_action?: string
}

export interface BranchInfo {
  name: string
  is_current: boolean
  is_remote: boolean
  last_commit_hash?: string
  last_commit_message?: string
}

export interface BranchSyncStatus {
  branch: string
  upstream?: string | null
  has_upstream: boolean
  ahead: number
  behind: number
  pending_local_commits?: number
}

export interface PendingPushFile {
  path: string
  commits_touching: number
}

export interface PendingPushPreview {
  branch: string
  commit_count: number
  files: PendingPushFile[]
  truncated?: boolean
}

export interface AuthenticatedUser {
  login: string
  name: string
  avatar_url: string
  group?: string
  is_admin: boolean
}

export interface GitGovConfig {
  branches: {
    patterns: string[]
    protected: string[]
  }
  groups: Record<string, GroupConfig>
  admins: string[]
  rules: RulesConfig
  checklist: ChecklistConfig
  enforcement: EnforcementConfig
}

export interface RulesConfig {
  require_pull_request: boolean
  min_approvals: number
  require_conventional_commits: boolean
  require_signed_commits: boolean
  max_files_per_commit: number | null
  require_linked_ticket: boolean
  block_force_push: boolean
  forbidden_patterns: string[]
}

export interface ChecklistConfig {
  confirm: string[]
  auto_check: string[]
}

export type EnforcementLevel = 'off' | 'warn' | 'block'

export interface EnforcementConfig {
  pull_requests: EnforcementLevel
  commits: EnforcementLevel
  branches: EnforcementLevel
  traceability: EnforcementLevel
}

export type GovernancePreset = 'startup' | 'enterprise' | 'regulated' | 'custom'

export interface RuleViolation {
  rule: string
  category: string
  enforcement: string
  message: string
}

export interface PolicyCheckResponse {
  advisory: boolean
  allowed: boolean
  reasons: string[]
  warnings: string[]
  evaluated_rules: string[]
  enforcement_applied: string
  violations: RuleViolation[]
}

export interface GroupConfig {
  members: string[]
  allowed_branches: string[]
  allowed_paths: string[]
}

export interface RepoValidation {
  path_exists: boolean
  is_git_repo: boolean
  has_remote_origin: boolean
  has_gitgov_toml: boolean
  remote_url?: string
}

export interface DeviceFlowInfo {
  user_code: string
  verification_uri: string
  device_code: string
  interval: number
}

export interface ValidationResult {
  type: 'Valid' | 'Blocked'
  message?: string
}

export interface PathValidationResult {
  path: string
  allowed: boolean
  reason?: string
}

export interface CommitMessageValidation {
  valid: boolean
  error?: string
}

// ============================================================================
// CLI EMBEDDED TERMINAL
// ============================================================================

export type TerminalLineType = 'command' | 'stdout' | 'stderr' | 'system' | 'gitgov'

export interface TerminalLine {
  id: string
  type: TerminalLineType
  text: string
  timestamp: number
}

export type CommandOrigin = 'button_click' | 'manual_input'

export interface CommandEntry {
  id: string
  command: string
  args: string[]
  origin: CommandOrigin
  cwd: string
  branch: string
  user_login: string
  started_at: number
  finished_at?: number
  exit_code?: number
  output_lines: TerminalLine[]
}

/** Payload emitted by Tauri for each line of CLI output. */
export interface CliOutputEvent {
  command_id: string
  line_type: TerminalLineType
  text: string
}

/** Payload emitted by Tauri when a CLI command finishes. */
export interface CliFinishedEvent {
  command_id: string
  exit_code: number
}

// ============================================================================
// PIPELINE VISUALIZER
// ============================================================================

export type PipelineNodeKind =
  | 'ticket'      // Jira ticket linked to branch
  | 'branch'      // Branch creation/checkout
  | 'commit'      // Individual commit
  | 'pr'          // Pull request
  | 'review'      // Code review status
  | 'merge'       // Merge into target branch
  | 'pipeline'    // Jenkins pipeline run
  | 'deploy'      // Deployment (future)

export type PipelineNodeStatus =
  | 'pending'
  | 'active'
  | 'success'
  | 'warning'
  | 'failed'

export interface PipelineNode {
  id: string
  kind: PipelineNodeKind
  status: PipelineNodeStatus
  label: string
  detail?: string
  branch: string
  timestamp: number
  /** True if this node was created in the current session */
  is_current_session: boolean
  /** Link to related entity (Jira URL, PR URL, Jenkins build URL) */
  url?: string
  /** For commits: short SHA */
  sha?: string
  /** For tickets: ticket key (e.g., PROJ-123) */
  ticket_key?: string
  /** For pipelines: build number */
  build_number?: number
}

export interface PipelineBranch {
  name: string
  is_current: boolean
  is_target: boolean  // develop or main
  nodes: PipelineNode[]
}

export interface PipelineGraph {
  branches: PipelineBranch[]
  session_start: number
}
