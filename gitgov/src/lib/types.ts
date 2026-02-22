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

export interface AuditFilter {
  start_date?: number
  end_date?: number
  developer_login?: string
  action?: string
  status?: string
  branch?: string
  limit: number
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
