import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { CombinedEvent } from '@/lib/types'

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

interface ControlPlaneState {
  serverConfig: ServerConfig | null
  serverStats: ServerStats | null
  serverLogs: CombinedEvent[]
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
  apiKeys: ApiKeyInfo[]
  isLoadingApiKeys: boolean
  exportLogs: ExportLogEntry[]
  isConnected: boolean
  isLoading: boolean
  isRefreshingDashboard: boolean
  error: string | null
}

interface ControlPlaneActions {
  initFromEnv: () => Promise<void>
  setServerConfig: (config: ServerConfig) => void
  checkConnection: () => Promise<void>
  refreshDashboardData: (params?: { logLimit?: number }) => Promise<void>
  loadStats: () => Promise<void>
  loadDailyActivity: (days?: number) => Promise<void>
  loadLogs: (limit?: number, offset?: number) => Promise<void>
  setLogsPage: (page: number) => void
  loadJenkinsCorrelations: (limit?: number) => Promise<void>
  loadPrMergeEvidence: (limit?: number) => Promise<void>
  loadTicketCoverage: (params?: { hours?: number; repo_full_name?: string; branch?: string; org_name?: string }) => Promise<void>
  applyTicketCoverageFilters: (filters: Partial<JiraCoverageFilters>) => Promise<void>
  correlateJiraTickets: (params?: { hours?: number; limit?: number; repo_full_name?: string; org_name?: string }) => Promise<JiraCorrelateResponse | null>
  loadJiraTicketDetail: (ticketId: string) => Promise<JiraTicketDetail | null>
  loadMe: () => Promise<void>
  loadApiKeys: () => Promise<void>
  revokeApiKey: (keyId: string) => Promise<boolean>
  exportAuditData: (params: { exportType?: string; startDate?: number; endDate?: number; orgName?: string }) => Promise<ExportResponse | null>
  loadExportLogs: () => Promise<void>
  clearError: () => void
  disconnect: () => void
}

const CONTROL_PLANE_CONFIG_STORAGE_KEY = 'gitgov.control_plane_config'
const JIRA_COVERAGE_FILTERS_STORAGE_KEY = 'gitgov.jira_coverage_filters'
const JIRA_TICKET_DETAIL_TTL_MS = 2 * 60 * 1000

// Compatibility fallback: existing desktop setups relied on this default key.
// Keep it as last-resort fallback so the dashboard/logs continue working.
const LEGACY_DEFAULT_API_KEY = '57f1ed59-371d-46ef-9fdf-508f59bc4963'

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
  apiKeys: [],
  isLoadingApiKeys: false,
  exportLogs: [],
  isConnected: false,
  isLoading: false,
  isRefreshingDashboard: false,
  error: null,

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
      set({ userRole: me.role })
    } catch {
      // Non-fatal: role detection failure doesn't break dashboard
    }
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
      apiKeys: [],
      isLoadingApiKeys: false,
      exportLogs: [],
      isRefreshingDashboard: false,
      error: null,
    })
  },
}))
