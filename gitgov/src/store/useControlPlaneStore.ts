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

interface TicketCoverageItem extends Record<string, unknown> {}

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

interface ControlPlaneState {
  serverConfig: ServerConfig | null
  serverStats: ServerStats | null
  serverLogs: CombinedEvent[]
  jenkinsCorrelations: CommitPipelineCorrelation[]
  ticketCoverage: TicketCoverageStats | null
  jiraCoverageFilters: JiraCoverageFilters
  jiraTicketDetails: Record<string, JiraTicketDetail | null>
  jiraTicketDetailFetchedAt: Record<string, number>
  jiraTicketDetailLoading: Record<string, boolean>
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
  loadLogs: (limit?: number) => Promise<void>
  loadJenkinsCorrelations: (limit?: number) => Promise<void>
  loadTicketCoverage: (params?: { hours?: number; repo_full_name?: string; branch?: string; org_name?: string }) => Promise<void>
  applyTicketCoverageFilters: (filters: Partial<JiraCoverageFilters>) => Promise<void>
  correlateJiraTickets: (params?: { hours?: number; limit?: number; repo_full_name?: string; org_name?: string }) => Promise<JiraCorrelateResponse | null>
  loadJiraTicketDetail: (ticketId: string) => Promise<JiraTicketDetail | null>
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
      return parsed.toString().replace(/\/$/, parsed.pathname === '/' && !trimmed.endsWith('/') ? '' : '/')
    }
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

export const useControlPlaneStore = create<ControlPlaneState & ControlPlaneActions>((set, get) => ({
  serverConfig: null,
  serverStats: null,
  serverLogs: [],
  jenkinsCorrelations: [],
  ticketCoverage: null,
  jiraCoverageFilters: readStoredJiraCoverageFilters(),
  jiraTicketDetails: {},
  jiraTicketDetailFetchedAt: {},
  jiraTicketDetailLoading: {},
  isConnected: false,
  isLoading: false,
  isRefreshingDashboard: false,
  error: null,

  initFromEnv: async () => {
    // Auto-connect with stored config, env vars, or compatibility fallback.
    const config = resolveServerConfig()
    persistServerConfig(config)
    set({ serverConfig: config })
    await get().checkConnection()
  },

  setServerConfig: (config) => {
    const merged = resolveServerConfig(config, get().serverConfig)
    persistServerConfig(merged)
    set({ serverConfig: merged })
    get().checkConnection()
  },

  checkConnection: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return

    set({ isLoading: true, error: null })
    try {
      const healthy = await tauriInvoke<boolean>('cmd_server_health', { config: serverConfig })
      set({ isConnected: healthy, isLoading: false })
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
        get().loadLogs(params?.logLimit ?? 50),
        get().loadJenkinsCorrelations(50),
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

  loadLogs: async (limit = 100) => {
    const { serverConfig } = get()
    if (!serverConfig) return
    try {
      const logs = await tauriInvoke<CombinedEvent[]>('cmd_server_get_logs', {
        config: serverConfig,
        filter: { limit, offset: 0 },
      })
      set({ serverLogs: logs })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

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

  clearError: () => set({ error: null }),

  disconnect: () => {
    persistServerConfig(null)
    set({
      serverConfig: null,
      isConnected: false,
      serverStats: null,
      serverLogs: [],
      jenkinsCorrelations: [],
      ticketCoverage: null,
      jiraCoverageFilters: readStoredJiraCoverageFilters(),
      jiraTicketDetails: {},
      jiraTicketDetailFetchedAt: {},
      jiraTicketDetailLoading: {},
      isRefreshingDashboard: false,
      error: null,
    })
  },
}))
