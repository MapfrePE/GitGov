import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { CombinedEvent } from '@/lib/types'
import { detectBrowserTimezone, persistTimezone, readStoredTimezone } from '@/lib/timezone'

interface ServerConfig {
  url: string
  api_key?: string
}

interface GitHubEventStats {
  total: number
  today: number
  pushes_today: number
  by_type: Record<string, number>
}

interface ClientEventStats {
  total: number
  today: number
  blocked_today: number
  desktop_pushes_today: number
  by_type: Record<string, number>
  by_status: Record<string, number>
}

interface ViolationStats {
  total: number
  unresolved: number
  critical: number
}

interface PipelineHealthStats {
  total_7d: number
  success_7d: number
  failure_7d: number
  aborted_7d: number
  unstable_7d: number
  avg_duration_ms_7d: number
  repos_with_failures_7d: number
}

interface ServerStats {
  github_events: GitHubEventStats
  client_events: ClientEventStats
  violations: ViolationStats
  pipeline?: PipelineHealthStats
  active_devs_week: number
  active_repos: number
}

interface DailyActivityPoint {
  day: string
  commits: number
  pushes: number
}

interface CommitPipelineRun {
  pipeline_event_id: string
  pipeline_id: string
  job_name: string
  status: string
  duration_ms?: number | null
  triggered_by?: string | null
  ingested_at: number
}

interface CommitPipelineCorrelation {
  commit_event_id: string
  commit_sha: string
  commit_message?: string | null
  commit_created_at: number
  user_login: string
  branch?: string | null
  repo_name?: string | null
  pipeline?: CommitPipelineRun | null
}

interface PrMergeEvidenceEntry {
  id: string
  org_id?: string | null
  org_name?: string | null
  repo_id?: string | null
  repo_full_name?: string | null
  delivery_id: string
  pr_number: number
  pr_title?: string | null
  author_login?: string | null
  merged_by_login?: string | null
  approvers: string[]
  approvals_count: number
  head_sha?: string | null
  base_branch?: string | null
  created_at: number
}

type TicketCoverageItem = Record<string, unknown>

interface TicketCoverageStats {
  org: string
  period: string
  total_commits: number
  commits_with_ticket: number
  coverage_percentage: number
  commits_without_ticket: TicketCoverageItem[]
  tickets_without_commits: TicketCoverageItem[]
}

interface JiraCorrelateResponse {
  scanned_commits: number
  correlations_created: number
  correlated_tickets: string[]
}

interface JiraTicketDetail {
  id: string
  org_id?: string | null
  ticket_id: string
  ticket_url?: string | null
  title?: string | null
  status?: string | null
  assignee?: string | null
  reporter?: string | null
  priority?: string | null
  ticket_type?: string | null
  related_commits: string[]
  related_prs: string[]
  related_branches: string[]
  created_at?: number | null
  updated_at?: number | null
  ingested_at: number
}

interface JiraTicketDetailResponse {
  found: boolean
  ticket?: JiraTicketDetail | null
}

interface JiraCoverageFilters {
  hours: number
  repo_full_name: string
  branch: string
}

interface ActiveDev7dEntry {
  user_login: string
  events: number
  last_seen: number
  suspicious_test_data: boolean
  sample_repo_empty_count: number
}

export interface ApiKeyInfo {
  id: string
  client_id: string
  role: string
  org_id: string | null
  created_at: number
  last_used: number | null
  is_active: boolean
}

interface MeResponse {
  client_id: string
  role: string
  org_id: string | null
}

interface RevokeApiKeyResponse {
  success: boolean
  message: string
}

export interface OrgUser {
  id: string
  org_id: string
  login: string
  display_name: string | null
  email: string | null
  role: string
  status: string
  created_by: string | null
  updated_by: string | null
  created_at: number
  updated_at: number
}

export interface OrgInvitation {
  id: string
  org_id: string
  invite_email: string | null
  invite_login: string | null
  role: string
  status: string
  invited_by: string
  accepted_by: string | null
  accepted_at: number | null
  revoked_by: string | null
  revoked_at: number | null
  expires_at: number
  created_at: number
  updated_at: number
}

interface CreateOrgResponse {
  org_id: string
  login: string
  created: boolean
}

interface CreateOrgUserResponse {
  user: OrgUser
  created: boolean
}

interface OrgUsersResponse {
  entries: OrgUser[]
  total: number
}

interface CreateOrgInvitationResponse {
  invitation: OrgInvitation
  invite_token: string
}

interface OrgInvitationsResponse {
  entries: OrgInvitation[]
  total: number
}

export interface AcceptOrgInvitationResponse {
  invitation: OrgInvitation
  client_id: string
  role: string
  org_id: string
  api_key: string
}

interface IssueOrgUserApiKeyResponse {
  api_key: string | null
  client_id: string
  error: string | null
}

export interface TeamRepoSummary {
  repo_name: string
  events: number
  commits: number
  pushes: number
  blocked_pushes: number
  last_seen: number
}

export interface TeamDeveloperOverview {
  login: string
  display_name: string | null
  email: string | null
  role: string
  status: string
  last_seen: number | null
  total_events: number
  commits: number
  pushes: number
  blocked_pushes: number
  repos_active_count: number
  repos: TeamRepoSummary[]
}

export interface TeamRepoOverview {
  repo_name: string
  developers_active: number
  total_events: number
  commits: number
  pushes: number
  blocked_pushes: number
  last_seen: number
}

interface TeamOverviewResponse {
  entries: TeamDeveloperOverview[]
  total: number
}

interface TeamReposResponse {
  entries: TeamRepoOverview[]
  total: number
}

export interface ExportResponse {
  id: string
  export_type: string
  record_count: number
  content_hash: string
  data?: unknown
  created_at: number
}

export interface ExportLogEntry {
  id: string
  org_id: string | null
  exported_by: string
  export_type: string
  date_range_start: number | null
  date_range_end: number | null
  filters: unknown
  record_count: number
  content_hash: string | null
  file_path: string | null
  created_at: number
}

// ── Chat interfaces ──────────────────────────────────────────────────────────

export interface ChatAskResponse {
  status: 'ok' | 'insufficient_data' | 'feature_not_available' | 'error'
  answer: string
  missing_capability?: string | null
  can_report_feature: boolean
  data_refs: string[]
}

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  response?: ChatAskResponse
  timestamp: number
}

interface ControlPlaneState {
  serverConfig: ServerConfig | null
  serverStats: ServerStats | null
  serverLogs: CombinedEvent[]
  activeDevs7d: ActiveDev7dEntry[]
  activeDevs7dUpdatedAt: number | null
  logsPage: number
  logsPageSize: number
  jenkinsCorrelations: CommitPipelineCorrelation[]
  prMergeEvidence: PrMergeEvidenceEntry[]
  dailyActivity: DailyActivityPoint[]
  ticketCoverage: TicketCoverageStats | null
  jiraCoverageFilters: JiraCoverageFilters
  jiraTicketDetails: Record<string, JiraTicketDetail | null>
  jiraTicketDetailFetchedAt: Record<string, number>
  jiraTicketDetailLoading: Record<string, boolean>
  userRole: string | null
  userOrgId: string | null
  selectedOrgName: string
  orgUsers: OrgUser[]
  orgUsersTotal: number
  orgInvitations: OrgInvitation[]
  orgInvitationsTotal: number
  lastGeneratedInviteToken: string | null
  teamOverview: TeamDeveloperOverview[]
  teamOverviewTotal: number
  teamRepos: TeamRepoOverview[]
  teamReposTotal: number
  teamWindowDays: number
  teamStatusFilter: '' | 'active' | 'disabled'
  apiKeys: ApiKeyInfo[]
  isLoadingApiKeys: boolean
  exportLogs: ExportLogEntry[]
  isConnected: boolean
  isLoading: boolean
  isRefreshingDashboard: boolean
  error: string | null
  chatMessages: ChatMessage[]
  isChatLoading: boolean
  displayTimezone: string
}

interface ControlPlaneActions {
  initFromEnv: () => Promise<void>
  setServerConfig: (config: ServerConfig) => void
  checkConnection: () => Promise<void>
  refreshDashboardData: (params?: { logLimit?: number }) => Promise<void>
  loadStats: () => Promise<void>
  loadDailyActivity: (days?: number) => Promise<void>
  loadLogs: (limit?: number, offset?: number) => Promise<void>
  loadActiveDevs7d: () => Promise<void>
  setLogsPage: (page: number) => void
  loadJenkinsCorrelations: (limit?: number) => Promise<void>
  loadPrMergeEvidence: (limit?: number) => Promise<void>
  loadTicketCoverage: (params?: { hours?: number; repo_full_name?: string; branch?: string; org_name?: string }) => Promise<void>
  applyTicketCoverageFilters: (filters: Partial<JiraCoverageFilters>) => Promise<void>
  correlateJiraTickets: (params?: { hours?: number; limit?: number; repo_full_name?: string; org_name?: string }) => Promise<JiraCorrelateResponse | null>
  loadJiraTicketDetail: (ticketId: string) => Promise<JiraTicketDetail | null>
  loadMe: () => Promise<void>
  createOrg: (payload: { login: string; name?: string }) => Promise<CreateOrgResponse | null>
  setSelectedOrgName: (orgName: string) => void
  loadOrgUsers: (params?: { orgName?: string; status?: string; limit?: number; offset?: number }) => Promise<void>
  upsertOrgUser: (payload: {
    orgName?: string
    login: string
    email?: string
    displayName?: string
    role?: string
    status?: string
  }) => Promise<OrgUser | null>
  updateOrgUserStatus: (userId: string, status: 'active' | 'disabled') => Promise<OrgUser | null>
  issueApiKeyForOrgUser: (userId: string) => Promise<IssueOrgUserApiKeyResponse | null>
  loadOrgInvitations: (params?: { orgName?: string; status?: string; limit?: number; offset?: number }) => Promise<void>
  createOrgInvitation: (payload: {
    orgName?: string
    inviteEmail?: string
    inviteLogin?: string
    role?: string
    expiresInDays?: number
  }) => Promise<CreateOrgInvitationResponse | null>
  resendOrgInvitation: (invitationId: string, expiresInDays?: number) => Promise<CreateOrgInvitationResponse | null>
  revokeOrgInvitation: (invitationId: string) => Promise<boolean>
  previewOrgInvitation: (token: string) => Promise<OrgInvitation | null>
  acceptOrgInvitation: (payload: { token: string; login?: string }) => Promise<AcceptOrgInvitationResponse | null>
  setTeamFilters: (filters: { days?: number; status?: '' | 'active' | 'disabled' }) => void
  loadTeamOverview: (params?: { orgName?: string; days?: number; status?: '' | 'active' | 'disabled'; limit?: number; offset?: number }) => Promise<void>
  loadTeamRepos: (params?: { orgName?: string; days?: number; limit?: number; offset?: number }) => Promise<void>
  refreshForCurrentRole: () => Promise<void>
  loadApiKeys: () => Promise<void>
  revokeApiKey: (keyId: string) => Promise<boolean>
  exportAuditData: (params: { exportType?: string; startDate?: number; endDate?: number; orgName?: string }) => Promise<ExportResponse | null>
  loadExportLogs: () => Promise<void>
  clearError: () => void
  disconnect: () => void
  chatAsk: (question: string, orgName?: string) => Promise<ChatAskResponse | null>
  reportFeature: (question: string, missingCapability?: string) => Promise<boolean>
  clearChatMessages: () => void
  setDisplayTimezone: (tz: string) => void
}

const CONTROL_PLANE_CONFIG_STORAGE_KEY = 'gitgov.control_plane_config'
const JIRA_COVERAGE_FILTERS_STORAGE_KEY = 'gitgov.jira_coverage_filters'
const JIRA_TICKET_DETAIL_TTL_MS = 2 * 60 * 1000

// Compatibility fallback: existing desktop setups relied on this default key.
// Keep it as last-resort fallback so the dashboard/logs continue working.
const LEGACY_DEFAULT_API_KEY = '57f1ed59-371d-46ef-9fdf-508f59bc4963'
const DEV_ACTIVITY_WINDOW_MS = 7 * 24 * 60 * 60 * 1000

function isLikelySyntheticLogin(login: string): boolean {
  return /^(alias_|erase_ok_|hb_user_|user_[0-9a-f]{6,}|test_?user|golden_?test|smoke|manual-check|victim_)/i.test(login)
}

function normalizeLoopbackUrl(url: string): string {
  const trimmed = url.trim()
  if (!trimmed) return trimmed

  try {
    const parsed = new URL(trimmed)
    if (parsed.hostname === 'localhost') {
      parsed.hostname = '127.0.0.1'
    }
    // Control Plane config must be a base URL only (scheme + host + optional port).
    // Strip path/query/hash so outbox and dashboard don't diverge (e.g. /docs, /health).
    parsed.pathname = '/'
    parsed.search = ''
    parsed.hash = ''
    return parsed.origin
  } catch {
    // Ignore invalid URLs here; validation happens later in Tauri/server calls.
  }

  return trimmed
}

function readStoredServerConfig(): ServerConfig | null {
  try {
    const raw = window.localStorage.getItem(CONTROL_PLANE_CONFIG_STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<ServerConfig>
    if (!parsed || typeof parsed.url !== 'string') return null
    return {
      url: parsed.url,
      api_key: typeof parsed.api_key === 'string' && parsed.api_key.trim() ? parsed.api_key : undefined,
    }
  } catch {
    return null
  }
}

function persistServerConfig(config: ServerConfig | null) {
  try {
    if (!config) {
      window.localStorage.removeItem(CONTROL_PLANE_CONFIG_STORAGE_KEY)
      return
    }
    window.localStorage.setItem(CONTROL_PLANE_CONFIG_STORAGE_KEY, JSON.stringify(config))
  } catch {
    // ignore storage errors
  }
}

function readStoredJiraCoverageFilters(): JiraCoverageFilters {
  try {
    const raw = window.localStorage.getItem(JIRA_COVERAGE_FILTERS_STORAGE_KEY)
    if (!raw) return { hours: 72, repo_full_name: '', branch: '' }
    const parsed = JSON.parse(raw) as Partial<JiraCoverageFilters>
    return {
      hours: typeof parsed.hours === 'number' && Number.isFinite(parsed.hours) ? parsed.hours : 72,
      repo_full_name: typeof parsed.repo_full_name === 'string' ? parsed.repo_full_name : '',
      branch: typeof parsed.branch === 'string' ? parsed.branch : '',
    }
  } catch {
    return { hours: 72, repo_full_name: '', branch: '' }
  }
}

function persistJiraCoverageFilters(filters: JiraCoverageFilters) {
  try {
    window.localStorage.setItem(JIRA_COVERAGE_FILTERS_STORAGE_KEY, JSON.stringify(filters))
  } catch {
    // ignore
  }
}

function resolveServerConfig(input?: Partial<ServerConfig> | null, previous?: ServerConfig | null): ServerConfig {
  const stored = readStoredServerConfig()
  const envUrl = normalizeLoopbackUrl(import.meta.env.VITE_SERVER_URL || '')
  const envApiKey = (import.meta.env.VITE_API_KEY || '').trim()
  const url =
    normalizeLoopbackUrl(input?.url ?? '') ||
    normalizeLoopbackUrl(previous?.url ?? '') ||
    envUrl ||
    normalizeLoopbackUrl(stored?.url ?? '') ||
    'http://127.0.0.1:3000'

  const apiKey =
    input?.api_key?.trim() ||
    previous?.api_key?.trim() ||
    envApiKey ||
    stored?.api_key?.trim() ||
    LEGACY_DEFAULT_API_KEY

  return {
    url: normalizeLoopbackUrl(url),
    api_key: apiKey || undefined,
  }
}

async function syncOutboxServerConfig(config: ServerConfig | null): Promise<void> {
  try {
    await tauriInvoke('cmd_server_sync_outbox', { config })
  } catch {
    // Non-fatal: dashboard connectivity should still work even if outbox sync fails.
  }
}

export const useControlPlaneStore = create<ControlPlaneState & ControlPlaneActions>((set, get) => ({
  serverConfig: null,
  serverStats: null,
  serverLogs: [],
  activeDevs7d: [],
  activeDevs7dUpdatedAt: null,
  logsPage: 0,
  logsPageSize: 10,
  jenkinsCorrelations: [],
  prMergeEvidence: [],
  dailyActivity: [],
  ticketCoverage: null,
  jiraCoverageFilters: readStoredJiraCoverageFilters(),
  jiraTicketDetails: {},
  jiraTicketDetailFetchedAt: {},
  jiraTicketDetailLoading: {},
  userRole: null,
  userOrgId: null,
  selectedOrgName: '',
  orgUsers: [],
  orgUsersTotal: 0,
  orgInvitations: [],
  orgInvitationsTotal: 0,
  lastGeneratedInviteToken: null,
  teamOverview: [],
  teamOverviewTotal: 0,
  teamRepos: [],
  teamReposTotal: 0,
  teamWindowDays: 30,
  teamStatusFilter: '',
  apiKeys: [],
  isLoadingApiKeys: false,
  exportLogs: [],
  isConnected: false,
  isLoading: false,
  isRefreshingDashboard: false,
  error: null,
  chatMessages: [],
  isChatLoading: false,
  displayTimezone: readStoredTimezone() || detectBrowserTimezone(),

  initFromEnv: async () => {
    // Auto-connect with stored config, env vars, or compatibility fallback.
    const config = resolveServerConfig()
    persistServerConfig(config)
    set({ serverConfig: config })
    await syncOutboxServerConfig(config)
    await get().checkConnection()
  },

  setServerConfig: (config) => {
    const merged = resolveServerConfig(config, get().serverConfig)
    persistServerConfig(merged)
    set({ serverConfig: merged })
    void syncOutboxServerConfig(merged)
    get().checkConnection()
  },

  checkConnection: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return

    set({ isLoading: true, error: null })
    try {
      const healthy = await tauriInvoke<boolean>('cmd_server_health', { config: serverConfig })
      set({ isConnected: healthy, isLoading: false })
      if (healthy) {
        void get().loadMe()
      }
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false, isConnected: false })
    }
  },

  refreshDashboardData: async (params) => {
    const { serverConfig, jiraCoverageFilters } = get()
    if (!serverConfig) return

    set({ isRefreshingDashboard: true })
    try {
      await Promise.all([
        get().loadStats(),
        get().loadDailyActivity(14),
        get().loadLogs(params?.logLimit ?? 50),
        get().loadActiveDevs7d(),
        get().loadJenkinsCorrelations(50),
        get().loadPrMergeEvidence(200),
        get().loadTicketCoverage({
          hours: jiraCoverageFilters.hours,
          repo_full_name: jiraCoverageFilters.repo_full_name.trim() || undefined,
          branch: jiraCoverageFilters.branch.trim() || undefined,
        }),
      ])
    } finally {
      set({ isRefreshingDashboard: false })
    }
  },

  loadStats: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return

    try {
      const stats = await tauriInvoke<ServerStats>('cmd_server_get_stats', { config: serverConfig })
      set({ serverStats: stats })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  loadDailyActivity: async (days = 14) => {
    const { serverConfig } = get()
    if (!serverConfig) return

    const safeDays = Number.isFinite(days) ? Math.max(1, Math.min(90, Math.floor(days))) : 14
    try {
      const points = await tauriInvoke<DailyActivityPoint[]>('cmd_server_get_daily_activity', {
        config: serverConfig,
        filter: { days: safeDays },
      })
      set({ dailyActivity: points })
    } catch {
      // Non-fatal: this widget should not break dashboard core flow.
    }
  },

  loadLogs: async (limit = 100, offset = 0) => {
    const { serverConfig } = get()
    if (!serverConfig) return
    try {
      const logs = await tauriInvoke<CombinedEvent[]>('cmd_server_get_logs', {
        config: serverConfig,
        filter: { limit, offset },
      })
      set({ serverLogs: logs })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  loadActiveDevs7d: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return

    const now = Date.now()
    const start = now - DEV_ACTIVITY_WINDOW_MS
    try {
      const logs = await tauriInvoke<CombinedEvent[]>('cmd_server_get_logs', {
        config: serverConfig,
        filter: {
          limit: 500,
          offset: 0,
          start_date: start,
          end_date: now,
        },
      })

      const grouped = new Map<string, {
        events: number
        last_seen: number
        sample_repo_empty_count: number
      }>()

      for (const log of logs) {
        const login = (log.user_login ?? '').trim()
        if (!login) continue
        const prev = grouped.get(login) ?? { events: 0, last_seen: 0, sample_repo_empty_count: 0 }
        prev.events += 1
        if (log.created_at > prev.last_seen) prev.last_seen = log.created_at
        if (!log.repo_name && !log.branch) prev.sample_repo_empty_count += 1
        grouped.set(login, prev)
      }

      const activeDevs7d: ActiveDev7dEntry[] = Array.from(grouped.entries())
        .map(([user_login, agg]) => {
          const allEmptyRepoBranch = agg.sample_repo_empty_count === agg.events
          return {
            user_login,
            events: agg.events,
            last_seen: agg.last_seen,
            suspicious_test_data: isLikelySyntheticLogin(user_login) || allEmptyRepoBranch,
            sample_repo_empty_count: agg.sample_repo_empty_count,
          }
        })
        .sort((a, b) => b.events - a.events || b.last_seen - a.last_seen)

      set({ activeDevs7d, activeDevs7dUpdatedAt: now })
    } catch {
      // Non-fatal fallback: keep existing list if request fails.
    }
  },

  setLogsPage: (page) => set({ logsPage: page }),

  loadJenkinsCorrelations: async (limit = 50) => {
    const { serverConfig } = get()
    if (!serverConfig) return

    try {
      const correlations = await tauriInvoke<CommitPipelineCorrelation[]>('cmd_server_get_jenkins_correlations', {
        config: serverConfig,
        filter: { limit, offset: 0 },
      })
      set({ jenkinsCorrelations: correlations })
    } catch {
      // Non-fatal for the dashboard core flow; leave existing data as-is.
    }
  },

  loadPrMergeEvidence: async (limit = 200) => {
    const { serverConfig } = get()
    if (!serverConfig) return

    try {
      const entries = await tauriInvoke<PrMergeEvidenceEntry[]>('cmd_server_get_pr_merges', {
        config: serverConfig,
        filter: { limit, offset: 0 },
      })
      set({ prMergeEvidence: entries })
    } catch {
      // Non-fatal: PR evidence is additive to the dashboard core flow.
    }
  },

  loadTicketCoverage: async (params) => {
    const { serverConfig } = get()
    if (!serverConfig) return

    const hours = params?.hours ?? 72
    try {
      const coverage = await tauriInvoke<TicketCoverageStats>('cmd_server_get_jira_ticket_coverage', {
        config: serverConfig,
        query: {
          hours,
          repo_full_name: params?.repo_full_name,
          branch: params?.branch,
          org_name: params?.org_name,
        },
      })
      set({ ticketCoverage: coverage })
    } catch {
      // Non-fatal for dashboard core flow
    }
  },

  applyTicketCoverageFilters: async (filters) => {
    const next = {
      ...get().jiraCoverageFilters,
      ...filters,
    }
    persistJiraCoverageFilters(next)
    set({ jiraCoverageFilters: next })
    await get().loadTicketCoverage({
      hours: next.hours,
      repo_full_name: next.repo_full_name || undefined,
      branch: next.branch || undefined,
    })
  },

  correlateJiraTickets: async (params) => {
    const { serverConfig } = get()
    if (!serverConfig) return null

    try {
      const response = await tauriInvoke<JiraCorrelateResponse>('cmd_server_correlate_jira_tickets', {
        config: serverConfig,
        request: {
          hours: params?.hours ?? 72,
          limit: params?.limit ?? 500,
          repo_full_name: params?.repo_full_name,
          org_name: params?.org_name,
        },
      })
      await get().loadTicketCoverage({
        hours: params?.hours ?? 72,
        repo_full_name: params?.repo_full_name,
        branch: undefined,
        org_name: params?.org_name,
      })
      return response
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  loadJiraTicketDetail: async (ticketId) => {
    const { serverConfig, jiraTicketDetails, jiraTicketDetailFetchedAt } = get()
    if (!serverConfig) return null
    const normalized = ticketId.trim().toUpperCase()
    if (!normalized) return null
    const fetchedAt = jiraTicketDetailFetchedAt[normalized] ?? 0
    const isFresh = Date.now() - fetchedAt < JIRA_TICKET_DETAIL_TTL_MS
    if (isFresh && Object.prototype.hasOwnProperty.call(jiraTicketDetails, normalized)) {
      return jiraTicketDetails[normalized] ?? null
    }
    set((state) => ({
      jiraTicketDetailLoading: {
        ...state.jiraTicketDetailLoading,
        [normalized]: true,
      },
    }))
    try {
      const resp = await tauriInvoke<JiraTicketDetailResponse>('cmd_server_get_jira_ticket_detail', {
        config: serverConfig,
        ticketId: normalized,
      })
      const ticket = resp.found ? resp.ticket ?? null : null
      set((state) => ({
        jiraTicketDetails: {
          ...state.jiraTicketDetails,
          [normalized]: ticket,
        },
        jiraTicketDetailFetchedAt: {
          ...state.jiraTicketDetailFetchedAt,
          [normalized]: Date.now(),
        },
        jiraTicketDetailLoading: {
          ...state.jiraTicketDetailLoading,
          [normalized]: false,
        },
      }))
      return ticket
    } catch {
      set((state) => ({
        jiraTicketDetails: {
          ...state.jiraTicketDetails,
          [normalized]: null,
        },
        jiraTicketDetailFetchedAt: {
          ...state.jiraTicketDetailFetchedAt,
          [normalized]: Date.now(),
        },
        jiraTicketDetailLoading: {
          ...state.jiraTicketDetailLoading,
          [normalized]: false,
        },
      }))
      return null
    }
  },

  exportAuditData: async (params) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      const result = await tauriInvoke<ExportResponse>('cmd_server_export', {
        config: serverConfig,
        exportType: params.exportType ?? 'events',
        startDate: params.startDate ?? null,
        endDate: params.endDate ?? null,
        orgName: params.orgName ?? null,
      })
      await get().loadExportLogs()
      return result
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  loadExportLogs: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return
    try {
      const logs = await tauriInvoke<ExportLogEntry[]>('cmd_server_list_exports', { config: serverConfig })
      set({ exportLogs: logs })
    } catch {
      // Non-fatal
    }
  },

  loadMe: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return
    try {
      const me = await tauriInvoke<MeResponse>('cmd_server_get_me', { config: serverConfig })
      set({ userRole: me.role, userOrgId: me.org_id ?? null })
    } catch {
      // Backward-compat fallback: older servers may not expose /me.
      // If /stats works, treat current key as admin.
      try {
        await tauriInvoke<ServerStats>('cmd_server_get_stats', { config: serverConfig })
        set({ userRole: 'Admin', userOrgId: null })
      } catch {
        // Last-resort default to developer view.
        set({ userRole: 'Developer', userOrgId: null })
      }
    }
  },

  createOrg: async (payload) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      const response = await tauriInvoke<CreateOrgResponse>('cmd_server_create_org', {
        config: serverConfig,
        payload: {
          login: payload.login.trim(),
          name: payload.name?.trim() || null,
        },
      })
      if (response.login) {
        set({ selectedOrgName: response.login })
      }
      return response
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  setSelectedOrgName: (orgName) => {
    set({ selectedOrgName: orgName.trim() })
  },

  loadOrgUsers: async (params) => {
    const { serverConfig, selectedOrgName } = get()
    if (!serverConfig) return
    const orgName = params?.orgName?.trim() || selectedOrgName.trim() || undefined
    try {
      const response = await tauriInvoke<OrgUsersResponse>('cmd_server_list_org_users', {
        config: serverConfig,
        orgName,
        status: params?.status ?? null,
        limit: params?.limit ?? 50,
        offset: params?.offset ?? 0,
      })
      set({ orgUsers: response.entries, orgUsersTotal: response.total })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  upsertOrgUser: async (payload) => {
    const { serverConfig, selectedOrgName } = get()
    if (!serverConfig) return null
    const orgName = payload.orgName?.trim() || selectedOrgName.trim() || undefined
    try {
      const response = await tauriInvoke<CreateOrgUserResponse>('cmd_server_create_org_user', {
        config: serverConfig,
        payload: {
          login: payload.login.trim(),
          email: payload.email?.trim() || null,
          display_name: payload.displayName?.trim() || null,
          role: payload.role ?? null,
          status: payload.status ?? null,
          org_name: orgName ?? null,
        },
      })
      await get().loadOrgUsers({ orgName })
      return response.user
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  updateOrgUserStatus: async (userId, status) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      const response = await tauriInvoke<OrgUser>('cmd_server_update_org_user_status', {
        config: serverConfig,
        userId,
        status,
      })
      await get().loadOrgUsers()
      return response
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  issueApiKeyForOrgUser: async (userId) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      const response = await tauriInvoke<IssueOrgUserApiKeyResponse>('cmd_server_create_api_key_for_org_user', {
        config: serverConfig,
        userId,
      })
      return response
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  loadOrgInvitations: async (params) => {
    const { serverConfig, selectedOrgName } = get()
    if (!serverConfig) return
    const orgName = params?.orgName?.trim() || selectedOrgName.trim() || undefined
    try {
      const response = await tauriInvoke<OrgInvitationsResponse>('cmd_server_list_org_invitations', {
        config: serverConfig,
        orgName,
        status: params?.status ?? null,
        limit: params?.limit ?? 50,
        offset: params?.offset ?? 0,
      })
      set({ orgInvitations: response.entries, orgInvitationsTotal: response.total })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  createOrgInvitation: async (payload) => {
    const { serverConfig, selectedOrgName } = get()
    if (!serverConfig) return null
    const orgName = payload.orgName?.trim() || selectedOrgName.trim() || undefined
    try {
      const response = await tauriInvoke<CreateOrgInvitationResponse>('cmd_server_create_org_invitation', {
        config: serverConfig,
        payload: {
          org_name: orgName ?? null,
          invite_email: payload.inviteEmail?.trim() || null,
          invite_login: payload.inviteLogin?.trim() || null,
          role: payload.role ?? null,
          expires_in_days: payload.expiresInDays ?? null,
        },
      })
      set({ lastGeneratedInviteToken: response.invite_token })
      await get().loadOrgInvitations({ orgName })
      await get().loadOrgUsers({ orgName })
      return response
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  resendOrgInvitation: async (invitationId, expiresInDays) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      const response = await tauriInvoke<CreateOrgInvitationResponse>('cmd_server_resend_org_invitation', {
        config: serverConfig,
        invitationId,
        expiresInDays: expiresInDays ?? null,
      })
      set({ lastGeneratedInviteToken: response.invite_token })
      await get().loadOrgInvitations()
      return response
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  revokeOrgInvitation: async (invitationId) => {
    const { serverConfig } = get()
    if (!serverConfig) return false
    try {
      await tauriInvoke<OrgInvitation>('cmd_server_revoke_org_invitation', {
        config: serverConfig,
        invitationId,
      })
      await get().loadOrgInvitations()
      return true
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return false
    }
  },

  previewOrgInvitation: async (token) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      const invite = await tauriInvoke<OrgInvitation>('cmd_server_preview_org_invitation', {
        config: serverConfig,
        token,
      })
      return invite
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  acceptOrgInvitation: async ({ token, login }) => {
    const { serverConfig } = get()
    if (!serverConfig) return null
    try {
      return await tauriInvoke<AcceptOrgInvitationResponse>('cmd_server_accept_org_invitation', {
        config: serverConfig,
        token,
        login: login?.trim() || null,
      })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return null
    }
  },

  setTeamFilters: (filters) => {
    set((state) => ({
      teamWindowDays: typeof filters.days === 'number' ? Math.max(1, Math.min(180, Math.floor(filters.days))) : state.teamWindowDays,
      teamStatusFilter: typeof filters.status === 'string' ? filters.status : state.teamStatusFilter,
    }))
  },

  loadTeamOverview: async (params) => {
    const { serverConfig, selectedOrgName, teamWindowDays, teamStatusFilter } = get()
    if (!serverConfig) return
    const orgName = params?.orgName?.trim() || selectedOrgName.trim() || undefined
    const days = typeof params?.days === 'number' ? params.days : teamWindowDays
    const status = params?.status ?? teamStatusFilter
    try {
      const response = await tauriInvoke<TeamOverviewResponse>('cmd_server_get_team_overview', {
        config: serverConfig,
        orgName,
        status: status || null,
        days,
        limit: params?.limit ?? 100,
        offset: params?.offset ?? 0,
      })
      set({
        teamOverview: response.entries,
        teamOverviewTotal: response.total,
      })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  loadTeamRepos: async (params) => {
    const { serverConfig, selectedOrgName, teamWindowDays } = get()
    if (!serverConfig) return
    const orgName = params?.orgName?.trim() || selectedOrgName.trim() || undefined
    const days = typeof params?.days === 'number' ? params.days : teamWindowDays
    try {
      const response = await tauriInvoke<TeamReposResponse>('cmd_server_get_team_repos', {
        config: serverConfig,
        orgName,
        days,
        limit: params?.limit ?? 100,
        offset: params?.offset ?? 0,
      })
      set({
        teamRepos: response.entries,
        teamReposTotal: response.total,
      })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  refreshForCurrentRole: async () => {
    const { userRole, selectedOrgName, teamWindowDays, teamStatusFilter } = get()
    if (userRole === 'Admin') {
      await get().refreshDashboardData({ logLimit: 50 })
      const scopedOrgName = selectedOrgName.trim() || undefined
      await Promise.all([
        get().loadOrgUsers({ orgName: scopedOrgName }),
        get().loadOrgInvitations({ orgName: scopedOrgName }),
        get().loadTeamOverview({ orgName: scopedOrgName, days: teamWindowDays, status: teamStatusFilter }),
        get().loadTeamRepos({ orgName: scopedOrgName, days: teamWindowDays }),
      ])
      return
    }

    await get().loadLogs(50, 0)
  },

  loadApiKeys: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return
    set({ isLoadingApiKeys: true })
    try {
      const keys = await tauriInvoke<ApiKeyInfo[]>('cmd_server_list_api_keys', { config: serverConfig })
      set({ apiKeys: keys })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    } finally {
      set({ isLoadingApiKeys: false })
    }
  },

  revokeApiKey: async (keyId) => {
    const { serverConfig } = get()
    if (!serverConfig) return false
    try {
      const resp = await tauriInvoke<RevokeApiKeyResponse>('cmd_server_revoke_api_key', {
        config: serverConfig,
        keyId,
      })
      if (resp.success) {
        await get().loadApiKeys()
      }
      return resp.success
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      return false
    }
  },

  clearError: () => set({ error: null }),

  disconnect: () => {
    persistServerConfig(null)
    void syncOutboxServerConfig(null)
    set({
      serverConfig: null,
      isConnected: false,
      serverStats: null,
      serverLogs: [],
      activeDevs7d: [],
      activeDevs7dUpdatedAt: null,
      logsPage: 0,
      jenkinsCorrelations: [],
      prMergeEvidence: [],
      dailyActivity: [],
      ticketCoverage: null,
      jiraCoverageFilters: readStoredJiraCoverageFilters(),
      jiraTicketDetails: {},
      jiraTicketDetailFetchedAt: {},
      jiraTicketDetailLoading: {},
      userRole: null,
      userOrgId: null,
      selectedOrgName: '',
      orgUsers: [],
      orgUsersTotal: 0,
      orgInvitations: [],
      orgInvitationsTotal: 0,
      lastGeneratedInviteToken: null,
      teamOverview: [],
      teamOverviewTotal: 0,
      teamRepos: [],
      teamReposTotal: 0,
      teamWindowDays: 30,
      teamStatusFilter: '',
      apiKeys: [],
      isLoadingApiKeys: false,
      exportLogs: [],
      isRefreshingDashboard: false,
      error: null,
      chatMessages: [],
      isChatLoading: false,
    })
  },

  // ── Chat actions ─────────────────────────────────────────────────────────

  chatAsk: async (question, orgName) => {
    const { serverConfig } = get()
    if (!serverConfig) return null

    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: question,
      timestamp: Date.now(),
    }
    set((s) => ({ chatMessages: [...s.chatMessages, userMsg], isChatLoading: true }))

    try {
      const response = await tauriInvoke<ChatAskResponse>('cmd_server_chat_ask', {
        config: serverConfig,
        request: { question, org_name: orgName ?? null },
      })
      const assistantMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response.answer,
        response,
        timestamp: Date.now(),
      }
      set((s) => ({ chatMessages: [...s.chatMessages, assistantMsg], isChatLoading: false }))
      return response
    } catch (e) {
      const errMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `Error: ${parseCommandError(String(e)).message}`,
        response: { status: 'error', answer: parseCommandError(String(e)).message, can_report_feature: false, data_refs: [] },
        timestamp: Date.now(),
      }
      set((s) => ({ chatMessages: [...s.chatMessages, errMsg], isChatLoading: false }))
      return null
    }
  },

  reportFeature: async (question, missingCapability) => {
    const { serverConfig, userOrgId } = get()
    if (!serverConfig) return false
    try {
      await tauriInvoke<{ id: string; status: string }>('cmd_server_create_feature_request', {
        config: serverConfig,
        input: {
          question,
          missing_capability: missingCapability ?? null,
          org_id: userOrgId ?? null,
          user_login: null,
          metadata: null,
        },
      })
      return true
    } catch {
      return false
    }
  },

  clearChatMessages: () => set({ chatMessages: [] }),

  setDisplayTimezone: (tz: string) => {
    persistTimezone(tz)
    set({ displayTimezone: tz })
  },
}))
