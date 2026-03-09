import { create } from 'zustand'
import { tauriInvoke, tauriListen, parseCommandError } from '@/lib/tauri'
import type { CombinedEvent } from '@/lib/types'
import { detectBrowserTimezone, persistTimezone, readStoredTimezone } from '@/lib/timezone'
import { useAuthStore } from '@/store/useAuthStore'
import { notifyNewEvents } from '@/lib/notifications'

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

interface PendingControlPlaneSession {
  client_id: string
  role: string
  org_id: string | null
  org_name: string | null
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

export interface ChatSession {
  id: string
  title: string
  created_at: number
  updated_at: number
  messages: ChatMessage[]
}

interface PolicyResponseData {
  version: string
  checksum: string
  config: import('@/lib/types').GitGovConfig
  updated_at: number
}

interface PolicyHistoryEntry {
  id: string
  repo_id: string
  config: import('@/lib/types').GitGovConfig
  checksum: string
  changed_by: string
  change_type: string
  previous_checksum: string | null
  created_at: number
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
  userClientId: string | null
  userOrgId: string | null
  controlPlaneAuthConfirmed: boolean
  pendingControlPlaneSession: PendingControlPlaneSession | null
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
  connectionStatus: 'connected' | 'disconnected' | 'maintenance' | 'checking'
  maintenanceDetectedAt: number | null
  isConnected: boolean
  isLoading: boolean
  isRefreshingDashboard: boolean
  error: string | null
  chatSessions: ChatSession[]
  activeChatSessionId: string | null
  chatMessages: ChatMessage[]
  isChatLoading: boolean
  displayTimezone: string
  policyData: PolicyResponseData | null
  policyHistory: PolicyHistoryEntry[]
  isPolicyLoading: boolean
  isPolicySaving: boolean
  policyError: string | null
  sseConnected: boolean
}

interface ControlPlaneActions {
  initFromEnv: () => Promise<void>
  setServerConfig: (config: ServerConfig) => void
  applyEnvApiKey: () => Promise<boolean>
  applyApiKey: (apiKey: string, url?: string) => Promise<boolean>
  markControlPlaneSessionValidated: (session: PendingControlPlaneSession) => void
  confirmControlPlaneSession: () => void
  resetControlPlaneAuthGate: () => void
  checkConnection: (options?: { background?: boolean }) => Promise<void>
  refreshDashboardData: (params?: { logLimit?: number; forceHeavy?: boolean }) => Promise<void>
  loadStats: () => Promise<void>
  loadDailyActivity: (days?: number) => Promise<void>
  loadLogs: (limit?: number, offset?: number) => Promise<void>
  loadLogsIncremental: (limit?: number) => Promise<void>
  loadActiveDevs7d: () => Promise<void>
  setLogsPage: (page: number) => void
  loadJenkinsCorrelations: (limit?: number) => Promise<void>
  loadPrMergeEvidence: (limit?: number) => Promise<void>
  loadTicketCoverage: (params?: { hours?: number; repo_full_name?: string; branch?: string; org_name?: string }) => Promise<void>
  applyTicketCoverageFilters: (filters: Partial<JiraCoverageFilters>) => Promise<void>
  correlateJiraTickets: (params?: { hours?: number; limit?: number; repo_full_name?: string; org_name?: string }) => Promise<JiraCorrelateResponse | null>
  loadJiraTicketDetail: (ticketId: string) => Promise<JiraTicketDetail | null>
  loadMe: () => Promise<boolean>
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
  loadTeamOverview: (params?: { orgName?: string; days?: number; status?: '' | 'active' | 'disabled'; limit?: number; offset?: number; append?: boolean }) => Promise<void>
  loadTeamRepos: (params?: { orgName?: string; days?: number; limit?: number; offset?: number; append?: boolean }) => Promise<void>
  refreshForCurrentRole: (options?: { forceHeavy?: boolean }) => Promise<void>
  loadApiKeys: () => Promise<void>
  revokeApiKey: (keyId: string) => Promise<boolean>
  exportAuditData: (params: { exportType?: string; startDate?: number; endDate?: number; orgName?: string }) => Promise<ExportResponse | null>
  loadExportLogs: () => Promise<void>
  clearError: () => void
  disconnect: () => void
  chatAsk: (question: string, orgName?: string) => Promise<ChatAskResponse | null>
  reportFeature: (question: string, missingCapability?: string) => Promise<boolean>
  clearChatMessages: () => void
  createChatSession: () => void
  setActiveChatSession: (sessionId: string) => void
  closeChatSession: (sessionId: string) => void
  refreshChatMessagesForActiveUser: () => void
  setDisplayTimezone: (tz: string) => void
  loadPolicy: (repoName: string) => Promise<void>
  savePolicy: (repoName: string, config: import('@/lib/types').GitGovConfig) => Promise<boolean>
  loadPolicyHistory: (repoName: string) => Promise<void>
  connectSse: () => Promise<void>
  disconnectSse: () => void
}

const CONTROL_PLANE_CONFIG_STORAGE_KEY = 'gitgov.control_plane_config'
const JIRA_COVERAGE_FILTERS_STORAGE_KEY = 'gitgov.jira_coverage_filters'
const LEGACY_CHAT_MESSAGES_STORAGE_KEY = 'gitgov.chat_messages'
const CHAT_MESSAGES_STORAGE_KEY_PREFIX = 'gitgov.chat_messages.v2.'
const JIRA_TICKET_DETAIL_TTL_MS = 2 * 60 * 1000
const DEV_LOCAL_SERVER_URL = 'http://127.0.0.1:3000'
const IS_DEV_MODE = Boolean(import.meta.env.DEV)
const FOUNDER_GITHUB_LOGIN = (
  import.meta.env.VITE_FOUNDER_GITHUB_LOGIN ||
  import.meta.env.VITE_FOUNDER_LOGIN ||
  ''
).trim()

// Compatibility fallback: can be provided explicitly via env when needed.
const LEGACY_DEFAULT_API_KEY = (import.meta.env.VITE_LEGACY_DEFAULT_API_KEY || '').trim()
const ALLOW_LEGACY_DEFAULT_API_KEY = (() => {
  const raw = (import.meta.env.VITE_ALLOW_LEGACY_DEFAULT_API_KEY || '').trim().toLowerCase()
  if (raw === '1' || raw === 'true' || raw === 'yes' || raw === 'on') return true
  if (raw === '0' || raw === 'false' || raw === 'no' || raw === 'off') return false
  return IS_DEV_MODE
})()
const DEV_ACTIVITY_WINDOW_MS = 7 * 24 * 60 * 60 * 1000
const HEAVY_DASHBOARD_REFRESH_MS = 5 * 60 * 1000
const JIRA_TICKET_CACHE_MAX = 50
const LOGS_KEYSET_PAGE_SIZE = 500
const LOGS_KEYSET_MAX_PAGES = 64
const MAX_CHAT_SESSIONS = 8
const MAX_CHAT_MESSAGES_PER_SESSION = 80
const DEFAULT_CHAT_SESSION_TITLE = 'Chat nuevo'
let cachedSecureControlPlaneApiKey: string | undefined

interface StoredChatStateV2 {
  version: 2
  active_session_id: string
  sessions: ChatSession[]
}

function isLikelySyntheticLogin(login: string): boolean {
  return /^(alias_|erase_ok_|hb_user_|user_[0-9a-f]{6,}|test_?user|golden_?test|smoke|manual-check|victim_)/i.test(login)
}

function buildActiveDevs7dFromLogs(logs: CombinedEvent[], now: number): ActiveDev7dEntry[] {
  const start = now - DEV_ACTIVITY_WINDOW_MS
  const grouped = new Map<string, {
    events: number
    last_seen: number
    sample_repo_empty_count: number
  }>()

  for (const log of logs) {
    if (log.created_at < start || log.created_at > now) continue
    const login = (log.user_login ?? '').trim()
    if (!login) continue
    const prev = grouped.get(login) ?? { events: 0, last_seen: 0, sample_repo_empty_count: 0 }
    prev.events += 1
    if (log.created_at > prev.last_seen) prev.last_seen = log.created_at
    if (!log.repo_name && !log.branch) prev.sample_repo_empty_count += 1
    grouped.set(login, prev)
  }

  return Array.from(grouped.entries())
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
}

function compareCombinedEventDesc(a: CombinedEvent, b: CombinedEvent): number {
  if (a.created_at !== b.created_at) return b.created_at - a.created_at
  return b.id.localeCompare(a.id)
}

function mergeRecentLogs(existing: CombinedEvent[], incoming: CombinedEvent[], limit: number): CombinedEvent[] {
  if (incoming.length === 0) {
    // Return same reference to avoid invalidating downstream useMemo hooks
    return existing.length <= limit ? existing : existing.slice(0, limit)
  }
  const merged = [...incoming, ...existing]
  merged.sort(compareCombinedEventDesc)

  const deduped: CombinedEvent[] = []
  const seen = new Set<string>()
  for (const item of merged) {
    if (seen.has(item.id)) continue
    seen.add(item.id)
    deduped.push(item)
    if (deduped.length >= limit) break
  }
  return deduped
}

interface LogsKeysetCursor {
  before_created_at: number
  before_id: string
}

function getOldestLogsCursor(events: CombinedEvent[]): LogsKeysetCursor | null {
  if (events.length === 0) return null
  const tail = events[events.length - 1]
  if (!tail?.id || !Number.isFinite(tail.created_at)) return null
  return {
    before_created_at: tail.created_at,
    before_id: tail.id,
  }
}

function sanitizeLogsWindow(limit: number, offset: number): { safeLimit: number; safeOffset: number } {
  const safeLimit = Number.isFinite(limit) ? Math.max(1, Math.min(500, Math.floor(limit))) : 500
  const safeOffset = Number.isFinite(offset) ? Math.max(0, Math.floor(offset)) : 0
  return { safeLimit, safeOffset }
}

async function fetchLogsByFilter(
  serverConfig: ServerConfig,
  filter: Record<string, unknown>,
): Promise<CombinedEvent[]> {
  return tauriInvoke<CombinedEvent[]>('cmd_server_get_logs', {
    config: serverConfig,
    filter,
  })
}

async function fetchLogsKeysetWindow(
  serverConfig: ServerConfig,
  limit: number,
  offset: number,
): Promise<CombinedEvent[]> {
  const { safeLimit, safeOffset } = sanitizeLogsWindow(limit, offset)
  if (safeOffset === 0) {
    return fetchLogsByFilter(serverConfig, { limit: safeLimit, offset: 0 })
  }

  let remainingOffset = safeOffset
  const collected: CombinedEvent[] = []
  let cursor: LogsKeysetCursor | null = null
  let pageCount = 0

  while (pageCount < LOGS_KEYSET_MAX_PAGES && collected.length < safeLimit) {
    pageCount += 1
    const requested = Math.min(LOGS_KEYSET_PAGE_SIZE, safeLimit + remainingOffset)
    const filter: Record<string, unknown> = {
      limit: requested,
      offset: 0,
    }
    if (cursor) {
      filter.before_created_at = cursor.before_created_at
      filter.before_id = cursor.before_id
    }

    const page = await fetchLogsByFilter(serverConfig, filter)
    if (page.length === 0) break

    let consumeFrom = 0
    if (remainingOffset > 0) {
      const skipped = Math.min(remainingOffset, page.length)
      remainingOffset -= skipped
      consumeFrom = skipped
    }
    if (consumeFrom < page.length) {
      collected.push(...page.slice(consumeFrom))
    }

    cursor = getOldestLogsCursor(page)
    if (!cursor || page.length < requested) break
  }

  // Compatibility fallback for very deep legacy offsets.
  if (remainingOffset > 0) {
    return fetchLogsByFilter(serverConfig, { limit: safeLimit, offset: safeOffset })
  }

  return collected.slice(0, safeLimit)
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
      // Legacy v1: api_key could still exist in localStorage and must be migrated to keyring.
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
    // Persist only non-secret fields in localStorage.
    window.localStorage.setItem(CONTROL_PLANE_CONFIG_STORAGE_KEY, JSON.stringify({
      url: config.url,
    }))
  } catch {
    // ignore storage errors
  }
}

function normalizeSecretValue(input: string | null | undefined): string | undefined {
  const normalized = (input ?? '').trim()
  return normalized || undefined
}

async function readSecureControlPlaneApiKey(): Promise<string | undefined> {
  try {
    const value = await tauriInvoke<string | null>('cmd_cp_get_api_key')
    const normalized = normalizeSecretValue(value)
    cachedSecureControlPlaneApiKey = normalized
    return normalized
  } catch {
    return cachedSecureControlPlaneApiKey
  }
}

async function persistSecureControlPlaneApiKey(apiKey?: string): Promise<void> {
  const normalized = normalizeSecretValue(apiKey)
  try {
    if (!normalized) {
      await tauriInvoke('cmd_cp_clear_api_key')
      cachedSecureControlPlaneApiKey = undefined
      return
    }
    await tauriInvoke('cmd_cp_set_api_key', { apiKey: normalized })
    cachedSecureControlPlaneApiKey = normalized
  } catch {
    // ignore secure storage failures to avoid blocking connectivity checks
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

function sanitizeChatMessages(raw: unknown): ChatMessage[] {
  if (!Array.isArray(raw)) return []
  return raw
    .filter((item): item is ChatMessage => {
      if (!item || typeof item !== 'object') return false
      const candidate = item as Partial<ChatMessage>
      return (
        typeof candidate.id === 'string' &&
        (candidate.role === 'user' || candidate.role === 'assistant') &&
        typeof candidate.content === 'string' &&
        typeof candidate.timestamp === 'number'
      )
    })
    .slice(-MAX_CHAT_MESSAGES_PER_SESSION)
}

function parseStoredChatMessages(raw: string | null): ChatMessage[] {
  if (!raw) return []
  try {
    return sanitizeChatMessages(JSON.parse(raw))
  } catch {
    return []
  }
}

function normalizeChatTitle(input: string): string {
  const compact = input.replace(/\s+/g, ' ').trim()
  if (!compact) return DEFAULT_CHAT_SESSION_TITLE
  if (compact.length <= 36) return compact
  return `${compact.slice(0, 36)}...`
}

function deriveSessionTitleFromQuestion(question: string): string {
  return normalizeChatTitle(question)
}

function buildChatSession(messages: ChatMessage[] = [], title?: string): ChatSession {
  const now = Date.now()
  return {
    id: crypto.randomUUID(),
    title: title?.trim() ? normalizeChatTitle(title) : DEFAULT_CHAT_SESSION_TITLE,
    created_at: now,
    updated_at: now,
    messages: messages.slice(-MAX_CHAT_MESSAGES_PER_SESSION),
  }
}

function sanitizeChatSession(input: unknown, fallbackIndex: number): ChatSession | null {
  if (!input || typeof input !== 'object') return null
  const candidate = input as Partial<ChatSession>
  if (typeof candidate.id !== 'string') return null
  const messages = sanitizeChatMessages(candidate.messages)
  const createdAt = typeof candidate.created_at === 'number' && Number.isFinite(candidate.created_at)
    ? candidate.created_at
    : Date.now()
  const updatedAt = typeof candidate.updated_at === 'number' && Number.isFinite(candidate.updated_at)
    ? candidate.updated_at
    : createdAt
  const inferredTitle =
    typeof candidate.title === 'string' && candidate.title.trim()
      ? candidate.title
      : (messages.find((m) => m.role === 'user')?.content ?? `${DEFAULT_CHAT_SESSION_TITLE} ${fallbackIndex + 1}`)
  return {
    id: candidate.id,
    title: normalizeChatTitle(inferredTitle),
    created_at: createdAt,
    updated_at: updatedAt,
    messages,
  }
}

function normalizeChatSessions(input: unknown): ChatSession[] {
  if (!Array.isArray(input)) return []
  const sessions: ChatSession[] = []
  for (let i = 0; i < input.length; i += 1) {
    const normalized = sanitizeChatSession(input[i], i)
    if (normalized) sessions.push(normalized)
  }
  sessions.sort((a, b) => a.created_at - b.created_at)
  return sessions.slice(-MAX_CHAT_SESSIONS)
}

function readStoredChatStateFromRaw(raw: string | null): { sessions: ChatSession[]; activeSessionId: string | null } {
  if (!raw) return { sessions: [], activeSessionId: null }
  try {
    const parsed = JSON.parse(raw) as StoredChatStateV2 | ChatMessage[]
    if (Array.isArray(parsed)) {
      const legacyMessages = sanitizeChatMessages(parsed)
      if (!legacyMessages.length) return { sessions: [], activeSessionId: null }
      const single = buildChatSession(legacyMessages, legacyMessages.find((m) => m.role === 'user')?.content)
      return { sessions: [single], activeSessionId: single.id }
    }
    if (!parsed || typeof parsed !== 'object') return { sessions: [], activeSessionId: null }
    const sessions = normalizeChatSessions((parsed as StoredChatStateV2).sessions)
    if (!sessions.length) return { sessions: [], activeSessionId: null }
    const requested = (parsed as StoredChatStateV2).active_session_id
    const activeSessionId = sessions.some((s) => s.id === requested) ? requested : sessions[sessions.length - 1].id
    return { sessions, activeSessionId }
  } catch {
    return { sessions: [], activeSessionId: null }
  }
}

function deriveActiveChatMessages(sessions: ChatSession[], activeSessionId: string | null): ChatMessage[] {
  if (!activeSessionId) return []
  return sessions.find((session) => session.id === activeSessionId)?.messages ?? []
}

function ensureAtLeastOneSession(sessions: ChatSession[], activeSessionId: string | null): { sessions: ChatSession[]; activeSessionId: string } {
  if (sessions.length > 0 && activeSessionId && sessions.some((s) => s.id === activeSessionId)) {
    return { sessions, activeSessionId }
  }
  if (sessions.length > 0) {
    return { sessions, activeSessionId: sessions[sessions.length - 1].id }
  }
  const session = buildChatSession()
  return { sessions: [session], activeSessionId: session.id }
}

function getActiveChatStorageKey(): string {
  const login = (useAuthStore.getState().user?.login ?? '').trim().toLowerCase()
  const encodedLogin = login ? encodeURIComponent(login) : 'anonymous'
  return `${CHAT_MESSAGES_STORAGE_KEY_PREFIX}${encodedLogin}`
}

function hasScopedChatStorageEntries(): boolean {
  try {
    for (let i = 0; i < window.localStorage.length; i += 1) {
      const key = window.localStorage.key(i)
      if (key?.startsWith(CHAT_MESSAGES_STORAGE_KEY_PREFIX)) return true
    }
  } catch {
    // ignore storage errors
  }
  return false
}

function readStoredChatState(): { sessions: ChatSession[]; activeSessionId: string } {
  try {
    const userScopedKey = getActiveChatStorageKey()
    const userScopedRaw = window.localStorage.getItem(userScopedKey)
    if (userScopedRaw !== null) {
      const current = readStoredChatStateFromRaw(userScopedRaw)
      return ensureAtLeastOneSession(current.sessions, current.activeSessionId)
    }
    const legacyRaw = window.localStorage.getItem(LEGACY_CHAT_MESSAGES_STORAGE_KEY)
    if (!legacyRaw) return ensureAtLeastOneSession([], null)

    // Migrate legacy global history only when no scoped histories exist yet.
    // This prevents old mixed history from leaking to additional users.
    if (hasScopedChatStorageEntries()) return ensureAtLeastOneSession([], null)

    const legacyMessages = parseStoredChatMessages(legacyRaw)
    const migrated = ensureAtLeastOneSession(
      legacyMessages.length
        ? [buildChatSession(legacyMessages, legacyMessages.find((m) => m.role === 'user')?.content)]
        : [],
      null,
    )
    try {
      window.localStorage.setItem(userScopedKey, JSON.stringify({
        version: 2,
        active_session_id: migrated.activeSessionId,
        sessions: migrated.sessions,
      } satisfies StoredChatStateV2))
      window.localStorage.removeItem(LEGACY_CHAT_MESSAGES_STORAGE_KEY)
    } catch {
      // ignore migration persistence errors
    }
    return migrated
  } catch {
    return ensureAtLeastOneSession([], null)
  }
}

let chatPersistTimeoutId: number | null = null
let chatPersistIdleId: number | null = null
let checkConnectionInFlight: Promise<void> | null = null
let sseUnlisteners: Array<() => void> = []
let sseReconnectTimer: ReturnType<typeof setTimeout> | null = null
let refreshForCurrentRoleInFlight: Promise<void> | null = null
let lastHeavyDashboardRefreshAt = 0
const initialChatState = readStoredChatState()

function clearPendingChatPersistJob() {
  if (chatPersistTimeoutId !== null) {
    window.clearTimeout(chatPersistTimeoutId)
    chatPersistTimeoutId = null
  }
  if (chatPersistIdleId !== null && typeof window.cancelIdleCallback === 'function') {
    window.cancelIdleCallback(chatPersistIdleId)
    chatPersistIdleId = null
  }
}

function persistChatState(sessions: ChatSession[], activeSessionId: string) {
  try {
    const userScopedKey = getActiveChatStorageKey()
    clearPendingChatPersistJob()

    const writeToStorage = () => {
      const compactSessions = sessions.slice(-MAX_CHAT_SESSIONS).map((session) => {
        const compactMessages = session.messages.slice(-MAX_CHAT_MESSAGES_PER_SESSION).map((msg) => {
          const trimmedContent = msg.content.length > 4000 ? `${msg.content.slice(0, 4000)}\n...[recortado para rendimiento]` : msg.content
          if (!msg.response) {
            return { ...msg, content: trimmedContent }
          }
          const trimmedAnswer =
            msg.response.answer.length > 4000
              ? `${msg.response.answer.slice(0, 4000)}\n...[recortado para rendimiento]`
              : msg.response.answer
          return {
            ...msg,
            content: trimmedContent,
            response: {
              ...msg.response,
              answer: trimmedAnswer,
              data_refs: msg.response.data_refs.slice(0, 12),
            },
          }
        })
        const fallbackTitle = compactMessages.find((m) => m.role === 'user')?.content ?? session.title
        return {
          ...session,
          title: normalizeChatTitle(session.title || fallbackTitle),
          messages: compactMessages,
        }
      })
      const payload: StoredChatStateV2 = {
        version: 2,
        active_session_id: activeSessionId,
        sessions: compactSessions,
      }
      try {
        window.localStorage.setItem(userScopedKey, JSON.stringify(payload))
      } catch {
        // ignore
      }
    }

    const schedulePersist = () => {
      chatPersistTimeoutId = null
      writeToStorage()
    }

    // Defer heavy serialization to idle/debounced time to avoid UI hitch while typing.
    if (typeof window.requestIdleCallback === 'function') {
      chatPersistIdleId = window.requestIdleCallback(() => {
        chatPersistIdleId = null
        schedulePersist()
      }, { timeout: 500 })
      return
    }
    chatPersistTimeoutId = window.setTimeout(schedulePersist, 120)
  } catch {
    // ignore
  }
}

function parseRetryAfterSeconds(message: string): number | null {
  const quoted = message.match(/"retry_after_seconds"\s*:\s*(\d+)/i)
  if (quoted) return Number.parseInt(quoted[1], 10)
  const plain = message.match(/retry[_ -]?after[_ -]?seconds?\s*[:=]?\s*(\d+)/i)
  if (plain) return Number.parseInt(plain[1], 10)
  return null
}

function formatChatErrorMessage(rawMessage: string): string {
  const isRateLimited =
    /429/.test(rawMessage) ||
    /RATE_LIMITED/i.test(rawMessage) ||
    /Too many requests/i.test(rawMessage)

  if (!isRateLimited) return rawMessage

  const retryAfter = parseRetryAfterSeconds(rawMessage)
  if (retryAfter && Number.isFinite(retryAfter) && retryAfter > 0) {
    return `El chat está recibiendo demasiadas solicitudes ahora. Reintenta en ${retryAfter} segundos.`
  }
  return 'El chat está recibiendo demasiadas solicitudes ahora. Reintenta en unos segundos.'
}

function resolveServerConfig(
  input?: Partial<ServerConfig> | null,
  previous?: ServerConfig | null,
  secureApiKey?: string | null,
): ServerConfig {
  const stored = readStoredServerConfig()
  const envUrl = normalizeLoopbackUrl(import.meta.env.VITE_SERVER_URL || '')
  const envApiKey = (import.meta.env.VITE_API_KEY || '').trim()
  const candidateUrl =
    normalizeLoopbackUrl(input?.url ?? '') ||
    normalizeLoopbackUrl(previous?.url ?? '') ||
    envUrl ||
    normalizeLoopbackUrl(stored?.url ?? '') ||
    DEV_LOCAL_SERVER_URL
  const url = IS_DEV_MODE ? DEV_LOCAL_SERVER_URL : normalizeLoopbackUrl(candidateUrl)

  const apiKey =
    input?.api_key?.trim() ||
    previous?.api_key?.trim() ||
    envApiKey ||
    secureApiKey?.trim() ||
    cachedSecureControlPlaneApiKey?.trim() ||
    stored?.api_key?.trim() ||
    (ALLOW_LEGACY_DEFAULT_API_KEY ? LEGACY_DEFAULT_API_KEY : '')

  return {
    url: normalizeLoopbackUrl(url),
    api_key: apiKey || undefined,
  }
}

function isUnauthorizedError(message: string): boolean {
  const normalized = message.toLowerCase()
  return normalized.includes('401') || normalized.includes('unauthorized') || normalized.includes('invalid or expired api key')
}

function isControlPlaneIdentityCompatible(
  clientId: string,
  githubLogin: string | null,
  role: string,
): boolean {
  if (!githubLogin) return true

  const cp = clientId.trim().toLowerCase()
  const gh = githubLogin.trim().toLowerCase()
  const normalizedRole = role.trim().toLowerCase()
  if (!cp || !gh) return false

  // Founder global key: if founder login is configured, enforce it; if not configured, allow.
  if (cp === 'bootstrap-admin') {
    if (!FOUNDER_GITHUB_LOGIN) return true
    return gh === FOUNDER_GITHUB_LOGIN.toLowerCase()
  }

  // Developers must always match GitHub login.
  if (normalizedRole === 'developer') {
    return cp === gh
  }

  // Admin/Architect/PM keys may target service users or scoped org admins.
  return true
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
  userClientId: null,
  userOrgId: null,
  controlPlaneAuthConfirmed: true,
  pendingControlPlaneSession: null,
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
  connectionStatus: 'disconnected',
  maintenanceDetectedAt: null,
  isConnected: false,
  isLoading: false,
  isRefreshingDashboard: false,
  error: null,
  chatSessions: initialChatState.sessions,
  activeChatSessionId: initialChatState.activeSessionId,
  chatMessages: deriveActiveChatMessages(initialChatState.sessions, initialChatState.activeSessionId),
  isChatLoading: false,
  displayTimezone: readStoredTimezone() || detectBrowserTimezone(),
  policyData: null,
  policyHistory: [],
  isPolicyLoading: false,
  isPolicySaving: false,
  policyError: null,
  sseConnected: false,

  initFromEnv: async () => {
    const secureApiKey = await readSecureControlPlaneApiKey()
    // Auto-connect with secure keyring storage, env vars, or compatibility fallback.
    const config = resolveServerConfig(undefined, undefined, secureApiKey)
    persistServerConfig(config)
    await persistSecureControlPlaneApiKey(config.api_key)
    set({ serverConfig: config })
    await syncOutboxServerConfig(config)
    await get().checkConnection()
  },

  setServerConfig: (config) => {
    const merged = resolveServerConfig(config, get().serverConfig, cachedSecureControlPlaneApiKey)
    persistServerConfig(merged)
    void persistSecureControlPlaneApiKey(merged.api_key)
    set({ serverConfig: merged })
    void syncOutboxServerConfig(merged)
    get().checkConnection()
  },

  applyEnvApiKey: async () => {
    const { serverConfig } = get()
    const envApiKey = (import.meta.env.VITE_API_KEY || '').trim()
    if (!envApiKey) {
      set({ error: 'No existe VITE_API_KEY en el entorno actual.' })
      return false
    }

    const next = resolveServerConfig(
      {
        url: serverConfig?.url ?? DEV_LOCAL_SERVER_URL,
        api_key: envApiKey,
      },
      serverConfig,
      cachedSecureControlPlaneApiKey,
    )
    persistServerConfig(next)
    await persistSecureControlPlaneApiKey(next.api_key)
    set({ serverConfig: next, error: null })
    await syncOutboxServerConfig(next)
    await get().checkConnection()
    const state = get()
    return state.isConnected && state.userRole === 'Admin'
  },

  applyApiKey: async (apiKey, url) => {
    const { serverConfig } = get()
    const normalizedKey = apiKey.trim()
    if (!normalizedKey) {
      set({ error: 'Ingresa una API key válida.' })
      return false
    }
    const next = resolveServerConfig(
      {
        url: url?.trim() || serverConfig?.url || DEV_LOCAL_SERVER_URL,
        api_key: normalizedKey,
      },
      serverConfig,
      cachedSecureControlPlaneApiKey,
    )
    persistServerConfig(next)
    await persistSecureControlPlaneApiKey(next.api_key)
    set({ serverConfig: next, error: null })
    await syncOutboxServerConfig(next)
    await get().checkConnection()
    const state = get()
    return state.isConnected && Boolean(state.userRole)
  },

  markControlPlaneSessionValidated: (session) => {
    set({
      pendingControlPlaneSession: session,
      controlPlaneAuthConfirmed: false,
    })
  },

  confirmControlPlaneSession: () => {
    set({
      controlPlaneAuthConfirmed: true,
      pendingControlPlaneSession: null,
      error: null,
    })
  },

  resetControlPlaneAuthGate: () => {
    set({
      controlPlaneAuthConfirmed: true,
      pendingControlPlaneSession: null,
    })
  },

  checkConnection: async (options) => {
    if (checkConnectionInFlight) {
      await checkConnectionInFlight
      return
    }

    const run = (async () => {
      const { serverConfig, isConnected: wasConnected } = get()
      if (!serverConfig) return
      const isBackground = Boolean(options?.background)

      if (!isBackground) {
        set({ isLoading: true, error: null, connectionStatus: 'checking' })
      }
      try {
        const healthy = await tauriInvoke<boolean>('cmd_server_health', { config: serverConfig })
        if (healthy) {
          let hasRoleContext = await get().loadMe()

          if (!hasRoleContext) {
            const envApiKey = (import.meta.env.VITE_API_KEY || '').trim()
            const currentApiKey = serverConfig.api_key?.trim() || ''
            if (envApiKey && envApiKey !== currentApiKey) {
              const recoveredConfig: ServerConfig = { ...serverConfig, api_key: envApiKey }
              persistServerConfig(recoveredConfig)
              await persistSecureControlPlaneApiKey(recoveredConfig.api_key)
              await syncOutboxServerConfig(recoveredConfig)
              set({ serverConfig: recoveredConfig })
              hasRoleContext = await get().loadMe()
            }
          }

          if (hasRoleContext) {
            set({
              isConnected: true,
              isLoading: false,
              connectionStatus: 'connected',
              maintenanceDetectedAt: null,
              error: isBackground ? get().error : null,
            })
          } else {
            set({
              isConnected: false,
              isLoading: false,
              connectionStatus: 'disconnected',
              maintenanceDetectedAt: null,
              userRole: null,
              userClientId: null,
              userOrgId: null,
              controlPlaneAuthConfirmed: true,
              pendingControlPlaneSession: null,
              error: get().error ?? (isBackground ? null : 'No se pudo autenticar con el Control Plane. Verifica la API key.'),
            })
          }
        } else {
          // Health endpoint returned false — treat as maintenance if was previously connected
          if (wasConnected) {
            set((s) => ({
              isConnected: false,
              isLoading: false,
              connectionStatus: 'maintenance',
              maintenanceDetectedAt: s.maintenanceDetectedAt ?? Date.now(),
            }))
          } else {
            set({ isConnected: false, isLoading: false, connectionStatus: 'disconnected' })
          }
        }
      } catch (e) {
        const errMsg = parseCommandError(String(e)).message
        // If previously connected and now failing → server is likely restarting (maintenance)
        if (wasConnected) {
          set((s) => ({
            error: errMsg,
            isLoading: false,
            isConnected: false,
            connectionStatus: 'maintenance',
            maintenanceDetectedAt: s.maintenanceDetectedAt ?? Date.now(),
          }))
        } else {
          set({ error: errMsg, isLoading: false, isConnected: false, connectionStatus: 'disconnected' })
        }
      }
    })()

    checkConnectionInFlight = run
    try {
      await run
    } finally {
      if (checkConnectionInFlight === run) checkConnectionInFlight = null
    }
  },

  refreshDashboardData: async (params) => {
    const { serverConfig, jiraCoverageFilters } = get()
    if (!serverConfig) return

    set({ isRefreshingDashboard: true })
    try {
      const runStartedAt = Date.now()
      await Promise.all([
        get().loadStats(),
        get().loadDailyActivity(14),
        get().loadLogsIncremental(params?.logLimit ?? 500),
      ])

      const shouldRunHeavyRefresh =
        Boolean(params?.forceHeavy) ||
        lastHeavyDashboardRefreshAt === 0 ||
        runStartedAt - lastHeavyDashboardRefreshAt >= HEAVY_DASHBOARD_REFRESH_MS

      if (shouldRunHeavyRefresh) {
        await Promise.all([
          get().loadJenkinsCorrelations(50),
          get().loadPrMergeEvidence(200),
          get().loadTicketCoverage({
            hours: jiraCoverageFilters.hours,
            repo_full_name: jiraCoverageFilters.repo_full_name.trim() || undefined,
            branch: jiraCoverageFilters.branch.trim() || undefined,
          }),
        ])
        lastHeavyDashboardRefreshAt = Date.now()
      }

      const now = Date.now()
      const activeDevs7d = buildActiveDevs7dFromLogs(get().serverLogs, now)
      set({ activeDevs7d, activeDevs7dUpdatedAt: now })
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

  loadLogs: async (limit = 500, offset = 0) => {
    const { serverConfig } = get()
    if (!serverConfig) return
    try {
      const logs = await fetchLogsKeysetWindow(serverConfig, limit, offset)
      set({ serverLogs: logs })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  loadLogsIncremental: async (limit = 500) => {
    const { serverConfig, serverLogs } = get()
    if (!serverConfig) return

    const safeLimit = Math.max(1, Math.min(500, Math.floor(limit)))
    if (serverLogs.length === 0) {
      await get().loadLogs(safeLimit, 0)
      return
    }

    const latestTs = serverLogs.reduce((max, log) => log.created_at > max ? log.created_at : max, 0)
    try {
      const incoming = await tauriInvoke<CombinedEvent[]>('cmd_server_get_logs', {
        config: serverConfig,
        filter: {
          limit: Math.min(200, safeLimit),
          offset: 0,
          start_date: latestTs,
        },
      })

      if (incoming.length === 0) return
      const merged = mergeRecentLogs(serverLogs, incoming, safeLimit)
      set({ serverLogs: merged })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      // Conservative fallback: recover with full window if incremental call fails.
      await get().loadLogs(safeLimit, 0)
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
      const activeDevs7d = buildActiveDevs7dFromLogs(logs, now)

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
      set((state) => {
        const nextDetails = { ...state.jiraTicketDetails, [normalized]: ticket }
        const nextFetchedAt = { ...state.jiraTicketDetailFetchedAt, [normalized]: Date.now() }
        const nextLoading = { ...state.jiraTicketDetailLoading, [normalized]: false }

        // Evict oldest entries when cache exceeds limit
        const keys = Object.keys(nextFetchedAt)
        if (keys.length > JIRA_TICKET_CACHE_MAX) {
          const sorted = keys.sort((a, b) => (nextFetchedAt[a] ?? 0) - (nextFetchedAt[b] ?? 0))
          const toRemove = sorted.slice(0, keys.length - JIRA_TICKET_CACHE_MAX)
          for (const k of toRemove) {
            delete nextDetails[k]
            delete nextFetchedAt[k]
            delete nextLoading[k]
          }
        }

        return {
          jiraTicketDetails: nextDetails,
          jiraTicketDetailFetchedAt: nextFetchedAt,
          jiraTicketDetailLoading: nextLoading,
        }
      })
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
    if (!serverConfig) return false
    try {
      const me = await tauriInvoke<MeResponse>('cmd_server_get_me', { config: serverConfig })
      const githubLogin = useAuthStore.getState().user?.login ?? null
      if (!isControlPlaneIdentityCompatible(me.client_id, githubLogin, me.role)) {
        const founderHint = me.client_id === 'bootstrap-admin'
          ? ' La key founder (bootstrap-admin) requiere sesión GitHub del founder configurado en VITE_FOUNDER_GITHUB_LOGIN.'
          : ''
        set({
          userRole: null,
          userClientId: null,
          userOrgId: null,
          controlPlaneAuthConfirmed: true,
          pendingControlPlaneSession: null,
          error: `La API key autenticó como '${me.client_id}', pero tu sesión GitHub es '${githubLogin ?? 'desconocida'}'.${founderHint}`,
        })
        return false
      }
      set({ userRole: me.role, userClientId: me.client_id, userOrgId: me.org_id ?? null, error: null })
      return true
    } catch (e) {
      const meError = parseCommandError(String(e)).message
      // Backward-compat fallback: older servers may not expose /me.
      // If /stats works, treat current key as admin.
      try {
        await tauriInvoke<ServerStats>('cmd_server_get_stats', { config: serverConfig })
        set({ userRole: 'Admin', userClientId: null, userOrgId: null, error: null })
        return true
      } catch {
        set({
          userRole: null,
          userClientId: null,
          userOrgId: null,
          error: isUnauthorizedError(meError)
            ? 'API key inválida o expirada para Control Plane. Usa la key Founder/Admin.'
            : meError,
        })
        return false
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
      set((state) => {
        if (!params?.append) {
          return {
            teamOverview: response.entries,
            teamOverviewTotal: response.total,
          }
        }

        const merged = [...state.teamOverview]
        const seen = new Set(merged.map((entry) => entry.login))
        for (const entry of response.entries) {
          if (seen.has(entry.login)) continue
          seen.add(entry.login)
          merged.push(entry)
        }
        return {
          teamOverview: merged,
          teamOverviewTotal: response.total,
        }
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
      set((state) => {
        if (!params?.append) {
          return {
            teamRepos: response.entries,
            teamReposTotal: response.total,
          }
        }

        const merged = [...state.teamRepos]
        const seen = new Set(merged.map((entry) => entry.repo_name))
        for (const entry of response.entries) {
          if (seen.has(entry.repo_name)) continue
          seen.add(entry.repo_name)
          merged.push(entry)
        }
        return {
          teamRepos: merged,
          teamReposTotal: response.total,
        }
      })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  refreshForCurrentRole: async (options) => {
    if (refreshForCurrentRoleInFlight) {
      await refreshForCurrentRoleInFlight
      if (options?.forceHeavy) {
        await get().refreshForCurrentRole({ forceHeavy: true })
      }
      return
    }

    const run = (async () => {
      const { userRole } = get()
      if (userRole === 'Admin') {
        await get().refreshDashboardData({ logLimit: 500, forceHeavy: options?.forceHeavy })
        return
      }

      await get().loadLogsIncremental(500)
    })()

    refreshForCurrentRoleInFlight = run
    try {
      await run
    } finally {
      if (refreshForCurrentRoleInFlight === run) refreshForCurrentRoleInFlight = null
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
    // Teardown SSE connection
    get().disconnectSse()
    persistServerConfig(null)
    void persistSecureControlPlaneApiKey(undefined)
    void syncOutboxServerConfig(null)
    set({
      serverConfig: null,
      isConnected: false,
      sseConnected: false,
      connectionStatus: 'disconnected',
      maintenanceDetectedAt: null,
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
      userClientId: null,
      userOrgId: null,
      controlPlaneAuthConfirmed: true,
      pendingControlPlaneSession: null,
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
      isChatLoading: false,
    })
  },

  // ── Chat actions ─────────────────────────────────────────────────────────

  chatAsk: async (question, orgName) => {
    const { serverConfig, selectedOrgName } = get()
    if (!serverConfig) return null
    const effectiveOrgName = orgName?.trim() || selectedOrgName.trim() || undefined
    const questionTrimmed = question.trim()
    if (!questionTrimmed) return null

    let sessionId = get().activeChatSessionId
    if (!sessionId) {
      const seeded = buildChatSession()
      sessionId = seeded.id
      set((s) => ({
        chatSessions: [...s.chatSessions, seeded].slice(-MAX_CHAT_SESSIONS),
        activeChatSessionId: seeded.id,
        chatMessages: seeded.messages,
      }))
    }

    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: questionTrimmed,
      timestamp: Date.now(),
    }

    set((s) => {
      const idx = s.chatSessions.findIndex((session) => session.id === sessionId)
      if (idx < 0) return { isChatLoading: true }
      const target = s.chatSessions[idx]
      const isFirstUserQuestion = !target.messages.some((m) => m.role === 'user')
      const nextSession: ChatSession = {
        ...target,
        title: isFirstUserQuestion ? deriveSessionTitleFromQuestion(questionTrimmed) : target.title,
        updated_at: Date.now(),
        messages: [...target.messages, userMsg].slice(-MAX_CHAT_MESSAGES_PER_SESSION),
      }
      const nextSessions = [...s.chatSessions]
      nextSessions[idx] = nextSession
      persistChatState(nextSessions, s.activeChatSessionId ?? nextSession.id)
      return {
        chatSessions: nextSessions,
        chatMessages: s.activeChatSessionId === nextSession.id ? nextSession.messages : s.chatMessages,
        isChatLoading: true,
      }
    })

    try {
      const response = await tauriInvoke<ChatAskResponse>('cmd_server_chat_ask', {
        config: serverConfig,
        request: { question: questionTrimmed, org_name: effectiveOrgName ?? null },
      })
      const assistantMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response.answer,
        response,
        timestamp: Date.now(),
      }
      set((s) => {
        const idx = s.chatSessions.findIndex((session) => session.id === sessionId)
        if (idx < 0) return { isChatLoading: false }
        const target = s.chatSessions[idx]
        const nextSession: ChatSession = {
          ...target,
          updated_at: Date.now(),
          messages: [...target.messages, assistantMsg].slice(-MAX_CHAT_MESSAGES_PER_SESSION),
        }
        const nextSessions = [...s.chatSessions]
        nextSessions[idx] = nextSession
        persistChatState(nextSessions, s.activeChatSessionId ?? nextSession.id)
        return {
          chatSessions: nextSessions,
          chatMessages: s.activeChatSessionId === nextSession.id ? nextSession.messages : s.chatMessages,
          isChatLoading: false,
        }
      })
      return response
    } catch (e) {
      const parsedError = parseCommandError(String(e))
      const userFacingError = formatChatErrorMessage(parsedError.message)
      const errMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `Error: ${userFacingError}`,
        response: { status: 'error', answer: userFacingError, can_report_feature: false, data_refs: [] },
        timestamp: Date.now(),
      }
      set((s) => {
        const idx = s.chatSessions.findIndex((session) => session.id === sessionId)
        if (idx < 0) return { isChatLoading: false }
        const target = s.chatSessions[idx]
        const nextSession: ChatSession = {
          ...target,
          updated_at: Date.now(),
          messages: [...target.messages, errMsg].slice(-MAX_CHAT_MESSAGES_PER_SESSION),
        }
        const nextSessions = [...s.chatSessions]
        nextSessions[idx] = nextSession
        persistChatState(nextSessions, s.activeChatSessionId ?? nextSession.id)
        return {
          chatSessions: nextSessions,
          chatMessages: s.activeChatSessionId === nextSession.id ? nextSession.messages : s.chatMessages,
          isChatLoading: false,
        }
      })
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

  clearChatMessages: () => {
    set((s) => {
      const activeId = s.activeChatSessionId
      if (!activeId) return {}
      const idx = s.chatSessions.findIndex((session) => session.id === activeId)
      if (idx < 0) return {}
      const target = s.chatSessions[idx]
      const nextSession: ChatSession = { ...target, messages: [], updated_at: Date.now(), title: target.title || DEFAULT_CHAT_SESSION_TITLE }
      const nextSessions = [...s.chatSessions]
      nextSessions[idx] = nextSession
      persistChatState(nextSessions, activeId)
      return { chatSessions: nextSessions, chatMessages: [] }
    })
  },

  createChatSession: () => {
    if (get().isChatLoading) return
    set((s) => {
      let nextSessions = [...s.chatSessions]
      if (nextSessions.length >= MAX_CHAT_SESSIONS) {
        const removableIdx = nextSessions.findIndex((session) => session.id !== s.activeChatSessionId)
        nextSessions.splice(removableIdx >= 0 ? removableIdx : 0, 1)
      }
      const newSession = buildChatSession([], `${DEFAULT_CHAT_SESSION_TITLE} ${nextSessions.length + 1}`)
      nextSessions = [...nextSessions, newSession]
      persistChatState(nextSessions, newSession.id)
      return {
        chatSessions: nextSessions,
        activeChatSessionId: newSession.id,
        chatMessages: [],
        isChatLoading: false,
      }
    })
  },

  setActiveChatSession: (sessionId) => {
    if (get().isChatLoading) return
    set((s) => {
      const target = s.chatSessions.find((session) => session.id === sessionId)
      if (!target) return {}
      persistChatState(s.chatSessions, target.id)
      return { activeChatSessionId: target.id, chatMessages: target.messages }
    })
  },

  closeChatSession: (sessionId) => {
    set((s) => {
      if (s.isChatLoading && s.activeChatSessionId === sessionId) return {}
      const idx = s.chatSessions.findIndex((session) => session.id === sessionId)
      if (idx < 0) return {}

      if (s.chatSessions.length <= 1) {
        const resetSession: ChatSession = { ...s.chatSessions[0], messages: [], updated_at: Date.now(), title: DEFAULT_CHAT_SESSION_TITLE }
        persistChatState([resetSession], resetSession.id)
        return {
          chatSessions: [resetSession],
          activeChatSessionId: resetSession.id,
          chatMessages: [],
          isChatLoading: false,
        }
      }

      const remaining = s.chatSessions.filter((session) => session.id !== sessionId)
      const nextActiveId = s.activeChatSessionId === sessionId
        ? remaining[Math.max(0, idx - 1)]?.id ?? remaining[0].id
        : (s.activeChatSessionId ?? remaining[0].id)
      const nextMessages = remaining.find((session) => session.id === nextActiveId)?.messages ?? []
      persistChatState(remaining, nextActiveId)
      return {
        chatSessions: remaining,
        activeChatSessionId: nextActiveId,
        chatMessages: nextMessages,
      }
    })
  },

  refreshChatMessagesForActiveUser: () => {
    const next = readStoredChatState()
    set({
      chatSessions: next.sessions,
      activeChatSessionId: next.activeSessionId,
      chatMessages: deriveActiveChatMessages(next.sessions, next.activeSessionId),
      isChatLoading: false,
    })
  },

  setDisplayTimezone: (tz: string) => {
    persistTimezone(tz)
    set({ displayTimezone: tz })
  },

  loadPolicy: async (repoName: string) => {
    const serverConfig = get().serverConfig
    if (!serverConfig) return
    set({ isPolicyLoading: true, policyError: null })
    try {
      const config = { url: serverConfig.url, api_key: serverConfig.api_key }
      const result = await tauriInvoke<PolicyResponseData | null>('cmd_server_get_policy', {
        config,
        repoName,
      })
      set({ policyData: result ?? null, isPolicyLoading: false })
    } catch (e) {
      const msg = parseCommandError(String(e))
      set({ policyError: msg.message, isPolicyLoading: false })
    }
  },

  savePolicy: async (repoName: string, policyConfig: import('@/lib/types').GitGovConfig) => {
    const serverConfig = get().serverConfig
    if (!serverConfig) return false
    set({ isPolicySaving: true, policyError: null })
    try {
      const config = { url: serverConfig.url, api_key: serverConfig.api_key }
      const result = await tauriInvoke<PolicyResponseData>('cmd_server_override_policy', {
        config,
        repoName,
        policyConfig,
      })
      set({ policyData: result, isPolicySaving: false })
      return true
    } catch (e) {
      const msg = parseCommandError(String(e))
      set({ policyError: msg.message, isPolicySaving: false })
      return false
    }
  },

  loadPolicyHistory: async (repoName: string) => {
    const serverConfig = get().serverConfig
    if (!serverConfig) return
    try {
      const config = { url: serverConfig.url, api_key: serverConfig.api_key }
      const history = await tauriInvoke<PolicyHistoryEntry[]>('cmd_server_get_policy_history', {
        config,
        repoName,
      })
      set({ policyHistory: history })
    } catch {
      // non-fatal
    }
  },

  connectSse: async () => {
    const { serverConfig, sseConnected } = get()
    if (!serverConfig || sseConnected) return

    // Debounce: track whether an SSE refresh is already scheduled
    let sseRefreshScheduled = false

    // Listen for SSE events from Tauri backend
    const unlistenEvent = await tauriListen<{ type: string; count?: number }>('gitgov:sse-event', (payload) => {
      const eventType = payload?.type
      if ((eventType === 'new_events' || eventType === 'stats_updated') && !sseRefreshScheduled) {
        // Debounce: batch rapid SSE notifications into a single refresh
        sseRefreshScheduled = true
        setTimeout(() => {
          sseRefreshScheduled = false
          void get().loadLogsIncremental(500)
          void get().loadStats()
        }, 200)
        // Desktop notification for new events (fire-and-forget)
        if (eventType === 'new_events' && payload.count) {
          void notifyNewEvents(payload.count)
        }
      }
      // heartbeat — no action needed
    })

    const unlistenConnected = await tauriListen<unknown>('gitgov:sse-connected', () => {
      set({ sseConnected: true })
    })

    const unlistenDisconnected = await tauriListen<unknown>('gitgov:sse-disconnected', () => {
      set({ sseConnected: false })
      unlistenEvent()
      unlistenConnected()
      unlistenDisconnected()
      sseUnlisteners = []
      // Auto-reconnect after 5s if still connected to server
      sseReconnectTimer = setTimeout(() => {
        sseReconnectTimer = null
        if (get().isConnected) {
          void get().connectSse()
        }
      }, 5000)
    })

    // Store unlisten functions for cleanup on disconnect
    sseUnlisteners = [unlistenEvent, unlistenConnected, unlistenDisconnected]

    // Fire-and-forget: the command runs until connection drops or is cancelled
    const config = { url: serverConfig.url, api_key: serverConfig.api_key }
    tauriInvoke('cmd_server_sse_connect', { config }).catch(() => {
      // Connection failed — clean up listeners
      set({ sseConnected: false })
      for (const fn of sseUnlisteners) fn()
      sseUnlisteners = []
    })
  },

  disconnectSse: () => {
    set({ sseConnected: false })
    // Cancel pending reconnect timer
    if (sseReconnectTimer !== null) {
      clearTimeout(sseReconnectTimer)
      sseReconnectTimer = null
    }
    // Signal the Tauri backend to stop the stream loop (bumps generation)
    tauriInvoke('cmd_server_sse_disconnect', {}).catch(() => {})
    for (const fn of sseUnlisteners) fn()
    sseUnlisteners = []
  },
}))
