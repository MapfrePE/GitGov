import { Fragment, useEffect, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Badge } from '@/components/shared/Badge'
import { Spinner } from '@/components/shared/Spinner'
import { TrendingUp, Users, AlertTriangle, Activity, Server, GitCommit, Workflow, BarChart3, Ticket, X, ChevronDown, ChevronRight, ExternalLink } from 'lucide-react'
import type { CombinedEvent } from '@/lib/types'

interface StatCardProps {
  icon: React.ReactNode
  label: string
  value: string | number
  color: 'brand' | 'success' | 'warning' | 'danger'
}

const iconBgClasses = {
  brand: 'from-brand-500/20 to-brand-600/10 text-brand-400',
  success: 'from-success-500/20 to-success-600/10 text-success-400',
  warning: 'from-warning-500/20 to-warning-600/10 text-warning-400',
  danger: 'from-danger-500/20 to-danger-600/10 text-danger-400',
}

function StatCard({ icon, label, value, color }: StatCardProps) {
  return (
    <div className="card group">
      <div className="flex items-start justify-between">
        <div className={`p-2.5 rounded-xl bg-linear-to-br ${iconBgClasses[color]}`}>{icon}</div>
      </div>
      <p className="text-3xl font-bold text-white mt-3 tracking-tight">{value}</p>
      <p className="text-xs text-surface-400 mt-1 font-medium">{label}</p>
    </div>
  )
}

function ProgressBar({ value, color = 'success' }: { value: string; color?: 'success' | 'brand' }) {
  const num = parseFloat(value) || 0
  const barColor = color === 'brand' ? 'bg-brand-500' : 'bg-success-500'
  return (
    <div className="h-2.5 bg-surface-700/50 rounded-full overflow-hidden">
      <div
        className={`h-full ${barColor} rounded-full transition-all duration-500`}
        style={{ width: `${Math.max(0, Math.min(100, num))}%` }}
      />
    </div>
  )
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return <h3 className="card-header">{children}</h3>
}

function readDetailString(log: CombinedEvent, key: string): string | null {
  const value = log.details?.[key]
  if (typeof value === 'string' && value.trim().length > 0) {
    return value
  }

  const metadata =
    log.details && typeof log.details === 'object'
      ? (log.details['metadata'] as Record<string, unknown> | undefined)
      : undefined

  const nested = metadata?.[key]
  if (typeof nested === 'string' && nested.trim().length > 0) {
    return nested
  }

  const legacyDetails =
    log.details && typeof log.details === 'object'
      ? (log.details['legacy_details'] as Record<string, unknown> | undefined)
      : undefined
  const legacyMetadata =
    legacyDetails && typeof legacyDetails === 'object'
      ? (legacyDetails['metadata'] as Record<string, unknown> | undefined)
      : undefined
  const nestedLegacy = legacyMetadata?.[key]
  return typeof nestedLegacy === 'string' && nestedLegacy.trim().length > 0 ? nestedLegacy : null
}

function getLogDetailPreview(log: CombinedEvent): string | null {
  if (log.event_type === 'commit') {
    return readDetailString(log, 'commit_message')
  }

  if (log.status === 'failed' || log.status === 'blocked') {
    return readDetailString(log, 'reason')
  }

  return null
}

function getShortCommitSha(log: CombinedEvent): string | null {
  const sha = readDetailString(log, 'commit_sha')
  return sha ? sha.slice(0, 7) : null
}

function extractTicketIdsFromCommitLog(log: CombinedEvent): string[] {
  const values = [readDetailString(log, 'commit_message'), log.branch ?? null]
    .filter((v): v is string => typeof v === 'string' && v.trim().length > 0)
  const regex = /\b([A-Z][A-Z0-9]{1,15}-\d{1,9})\b/g
  const result: string[] = []
  const seen = new Set<string>()

  for (const value of values) {
    let match: RegExpExecArray | null
    regex.lastIndex = 0
    while ((match = regex.exec(value)) !== null) {
      const ticket = match[1].toUpperCase()
      if (!seen.has(ticket)) {
        seen.add(ticket)
        result.push(ticket)
      }
    }
  }
  return result
}

function formatDurationMs(ms?: number): string {
  if (!ms || ms <= 0) return '-'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes <= 0) return `${seconds}s`
  return `${minutes}m ${seconds}s`
}

function readDetailFiles(log: CombinedEvent): string[] {
  const direct = log.details?.['files']
  if (Array.isArray(direct)) {
    return direct.filter((v): v is string => typeof v === 'string')
  }
  return []
}

interface DashboardRow {
  log: CombinedEvent
  attachedFiles: string[]
}

function buildDashboardRows(logs: CombinedEvent[]): DashboardRow[] {
  const rows: DashboardRow[] = []
  const consumedStageFileIds = new Set<string>()

  for (let i = 0; i < logs.length; i++) {
    const log = logs[i]

    if (log.event_type === 'stage_files') {
      continue
    }

    if (log.event_type !== 'commit') {
      continue
    }

    let attachedFiles: string[] = []

    for (let j = i + 1; j < logs.length; j++) {
      const candidate = logs[j]
      if (candidate.event_type !== 'stage_files') continue
      if (consumedStageFileIds.has(candidate.id)) continue
      if ((candidate.user_login ?? '') !== (log.user_login ?? '')) continue

      const deltaMs = log.created_at - candidate.created_at
      if (deltaMs < 0) continue
      if (deltaMs > 10 * 60 * 1000) break

      const files = readDetailFiles(candidate)
      if (files.length > 0) {
        attachedFiles = files
        consumedStageFileIds.add(candidate.id)
      }
      break
    }

    rows.push({ log, attachedFiles })
  }

  return rows
}

export function ServerDashboard() {
  const {
    serverStats,
    serverLogs,
    jenkinsCorrelations,
    ticketCoverage,
    jiraCoverageFilters,
    jiraTicketDetails,
    jiraTicketDetailLoading,
    isConnected,
    isRefreshingDashboard,
    refreshDashboardData,
    loadLogs,
    applyTicketCoverageFilters,
    correlateJiraTickets,
    loadJiraTicketDetail,
  } = useControlPlaneStore()
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [expandedCommitRows, setExpandedCommitRows] = useState<Record<string, boolean>>({})
  const [isCorrelatingJira, setIsCorrelatingJira] = useState(false)
  const [ticketHours, setTicketHours] = useState(jiraCoverageFilters.hours)
  const [ticketRepoFilter, setTicketRepoFilter] = useState(jiraCoverageFilters.repo_full_name)
  const [ticketBranchFilter, setTicketBranchFilter] = useState(jiraCoverageFilters.branch)
  const [selectedTicketId, setSelectedTicketId] = useState<string | null>(null)
  const [ticketPanelExpanded, setTicketPanelExpanded] = useState(false)

  useEffect(() => {
    if (!isConnected) return

    void refreshDashboardData({ logLimit: 50 })

    if (!autoRefresh) return
    const interval = setInterval(() => {
      void refreshDashboardData({ logLimit: 50 })
    }, 30000)

    return () => clearInterval(interval)
  }, [isConnected, autoRefresh, refreshDashboardData])

  useEffect(() => {
    setTicketHours(jiraCoverageFilters.hours)
    setTicketRepoFilter(jiraCoverageFilters.repo_full_name)
    setTicketBranchFilter(jiraCoverageFilters.branch)
  }, [jiraCoverageFilters])

  useEffect(() => {
    if (!selectedTicketId) return
    setTicketPanelExpanded(false)
    void loadJiraTicketDetail(selectedTicketId)
  }, [selectedTicketId, loadJiraTicketDetail])

  if (!isConnected) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-surface-500 animate-fade-in">
        <Server size={48} className="mb-4 text-surface-600" />
        <p className="text-sm font-medium">Conecta al Control Plane</p>
        <p className="text-xs text-surface-600 mt-1">Configura la conexión para ver el dashboard</p>
      </div>
    )
  }

  const successRate = serverStats
    ? serverStats.github_events.pushes_today + serverStats.client_events.blocked_today > 0
      ? ((serverStats.github_events.pushes_today / (serverStats.github_events.pushes_today + serverStats.client_events.blocked_today)) * 100).toFixed(1)
      : '100.0'
    : '0'
  const pipeline = serverStats?.pipeline
  const pipelineTotal = pipeline?.total_7d ?? 0
  const pipelineSuccess = pipeline?.success_7d ?? 0
  const pipelineSuccessRate = pipelineTotal > 0 ? ((pipelineSuccess / pipelineTotal) * 100).toFixed(1) : '0.0'
  const commitsWithoutTicket = (ticketCoverage?.commits_without_ticket ?? []).slice(0, 5)
  const selectedTicketDetails = selectedTicketId
    ? (jiraTicketDetails[selectedTicketId] ??
      (ticketCoverage?.tickets_without_commits ?? []).find(
        (t) => typeof t.ticket_id === 'string' && t.ticket_id === selectedTicketId,
      ) ??
      null)
    : null
  const isSelectedTicketLoading = selectedTicketId ? !!jiraTicketDetailLoading[selectedTicketId] : false
  const ticketPanelSummaryText = isSelectedTicketLoading
    ? 'Cargando detalle de Jira...'
    : selectedTicketDetails &&
        typeof selectedTicketDetails === 'object' &&
        'title' in selectedTicketDetails &&
        typeof selectedTicketDetails.title === 'string' &&
        selectedTicketDetails.title
      ? selectedTicketDetails.title
      : selectedTicketDetails
        ? 'Detalle parcial desde coverage (ticket sin commits en el período filtrado).'
        : 'Ticket detectado en commit/rama. Para más detalle, ingiere Jira y ejecuta correlación con filtros compatibles.'
  const dashboardRows = buildDashboardRows(serverLogs).slice(0, 10)
  const pipelineByCommitSha = new Map(
    jenkinsCorrelations
      .filter((c) => c.pipeline && c.commit_sha)
      .map((c) => [c.commit_sha.toLowerCase(), c.pipeline!]),
  )

  const findPipelineForLog = (log: CombinedEvent) => {
    const sha = readDetailString(log, 'commit_sha')
    if (!sha) return null
    const normalized = sha.toLowerCase()
    const exact = pipelineByCommitSha.get(normalized)
    if (exact) return exact
    for (const [fullSha, pipelineRun] of pipelineByCommitSha.entries()) {
      if (fullSha.startsWith(normalized) || normalized.startsWith(fullSha)) {
        return pipelineRun
      }
    }
    return null
  }

  const refreshAll = () => {
    void refreshDashboardData({ logLimit: 50 })
  }

  return (
    <div className="space-y-6 animate-fade-in">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-bold text-white tracking-tight">Dashboard</h2>
          <p className="text-xs text-surface-500 mt-0.5">Control Plane overview</p>
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 text-xs text-surface-400 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
              className="rounded border-surface-600 bg-surface-800 text-brand-500 focus:ring-brand-500/30"
            />
            Auto-refresh
          </label>
          <Button variant="ghost" size="sm" onClick={refreshAll} loading={isRefreshingDashboard}>
            Actualizar
          </Button>
        </div>
      </div>

      {serverStats && (
        <>
          {/* Stat Cards */}
          <div className="grid grid-cols-4 gap-4">
            <StatCard
              icon={<TrendingUp size={18} />}
              label="Total Eventos GitHub"
              value={serverStats.github_events.total}
              color="brand"
            />
            <StatCard
              icon={<Activity size={18} />}
              label="Pushes Hoy"
              value={serverStats.github_events.pushes_today}
              color="success"
            />
            <StatCard
              icon={<AlertTriangle size={18} />}
              label="Bloqueados Hoy"
              value={serverStats.client_events.blocked_today}
              color="danger"
            />
            <StatCard
              icon={<Users size={18} />}
              label="Devs Activos (7d)"
              value={serverStats.active_devs_week}
              color="warning"
            />
          </div>

          {/* Success Rate + Repos */}
          <div className="grid grid-cols-2 gap-4">
            <div className="card">
              <SectionHeader>Tasa de Éxito</SectionHeader>
              <div className="flex items-center gap-4">
                <div className="text-3xl font-bold text-success-400 tracking-tight">{successRate}%</div>
                <div className="flex-1">
                  <ProgressBar value={successRate} />
                </div>
              </div>
            </div>

            <div className="card">
              <SectionHeader>Repos Activos</SectionHeader>
              <div className="flex items-center gap-4">
                <div className="text-3xl font-bold text-brand-400 tracking-tight">{serverStats.active_repos}</div>
                <span className="text-xs text-surface-500">últimos 7 días</span>
              </div>
            </div>
          </div>

          {/* Pipeline + Ticket Coverage */}
          <div className="grid grid-cols-2 gap-4">
            <div className="card">
              <SectionHeader>Pipeline Health (7d)</SectionHeader>
              {pipelineTotal > 0 ? (
                <div className="space-y-3">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-surface-400">Success Rate</span>
                    <span className="text-success-400 font-bold">{pipelineSuccessRate}%</span>
                  </div>
                  <ProgressBar value={pipelineSuccessRate} />
                  <div className="grid grid-cols-2 gap-3 text-sm pt-1">
                    <div className="flex items-center gap-2">
                      <Workflow size={13} className="text-surface-500" />
                      <span className="text-surface-400">Pipelines:</span>
                      <span className="text-white font-medium">{pipelineTotal}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <AlertTriangle size={13} className="text-danger-400" />
                      <span className="text-surface-400">Failures:</span>
                      <span className="text-danger-400 font-medium">{pipeline?.failure_7d ?? 0}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <BarChart3 size={13} className="text-surface-500" />
                      <span className="text-surface-400">Avg:</span>
                      <span className="text-white font-medium">{formatDurationMs(pipeline?.avg_duration_ms_7d)}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <Server size={13} className="text-surface-500" />
                      <span className="text-surface-400">Repos:</span>
                      <span className="text-white font-medium">{pipeline?.repos_with_failures_7d ?? 0}</span>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="py-4 text-center">
                  <Workflow size={28} className="mx-auto text-surface-600 mb-2" />
                  <p className="text-sm text-surface-500">Sin datos de pipelines</p>
                </div>
              )}
            </div>

            <div className="card">
              <div className="flex items-center justify-between mb-3">
                <SectionHeader>Ticket Coverage (Jira)</SectionHeader>
                <Button
                  variant="ghost"
                  size="sm"
                  loading={isCorrelatingJira}
                  onClick={async () => {
                    setIsCorrelatingJira(true)
                    try {
                      await correlateJiraTickets({
                        hours: jiraCoverageFilters.hours,
                        limit: 500,
                        repo_full_name: jiraCoverageFilters.repo_full_name.trim() || undefined,
                      })
                      await loadLogs(50)
                    } finally {
                      setIsCorrelatingJira(false)
                    }
                  }}
                >
                  Correlacionar
                </Button>
              </div>
              <div className="flex gap-2 mb-3">
                <input
                  value={ticketRepoFilter}
                  onChange={(e) => setTicketRepoFilter(e.target.value)}
                  placeholder="repo"
                  className="flex-1 rounded-lg bg-surface-900 border border-surface-700 px-2.5 py-1.5 text-xs text-white placeholder:text-surface-600 focus:border-brand-500 focus:ring-1 focus:ring-brand-500/20 focus:outline-none transition-all"
                />
                <input
                  value={ticketBranchFilter}
                  onChange={(e) => setTicketBranchFilter(e.target.value)}
                  placeholder="rama"
                  className="flex-1 rounded-lg bg-surface-900 border border-surface-700 px-2.5 py-1.5 text-xs text-white placeholder:text-surface-600 focus:border-brand-500 focus:ring-1 focus:ring-brand-500/20 focus:outline-none transition-all"
                />
                <select
                  value={ticketHours}
                  onChange={(e) => setTicketHours(Number(e.target.value))}
                  className="rounded-lg bg-surface-900 border border-surface-700 px-2.5 py-1.5 text-xs text-white focus:border-brand-500 focus:outline-none transition-all"
                >
                  <option value={24}>24h</option>
                  <option value={72}>72h</option>
                  <option value={168}>7d</option>
                  <option value={720}>30d</option>
                </select>
              </div>
              <div className="flex items-center justify-end gap-2 mb-3">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setTicketHours(72)
                    setTicketRepoFilter('')
                    setTicketBranchFilter('')
                  }}
                >
                  Limpiar
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => applyTicketCoverageFilters({
                    hours: ticketHours,
                    repo_full_name: ticketRepoFilter.trim(),
                    branch: ticketBranchFilter.trim(),
                  })}
                >
                  Aplicar
                </Button>
              </div>
              {ticketCoverage && ticketCoverage.total_commits > 0 ? (
                <div className="space-y-3">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-surface-400">Cobertura</span>
                    <span className="text-brand-400 font-bold">
                      {ticketCoverage.coverage_percentage.toFixed(1)}%
                    </span>
                  </div>
                  <ProgressBar value={ticketCoverage.coverage_percentage.toFixed(1)} color="brand" />
                  <div className="grid grid-cols-2 gap-2 text-sm pt-1">
                    <div className="text-surface-400">
                      Commits: <span className="text-white font-medium">{ticketCoverage.total_commits}</span>
                    </div>
                    <div className="text-surface-400">
                      Con ticket: <span className="text-success-400 font-medium">{ticketCoverage.commits_with_ticket}</span>
                    </div>
                    <div className="text-surface-400">
                      Sin ticket: <span className="text-warning-400 font-medium">{ticketCoverage.commits_without_ticket.length}</span>
                    </div>
                    <div className="text-surface-400">
                      Huérfanos: <span className="text-white font-medium">{ticketCoverage.tickets_without_commits.length}</span>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="py-4 text-center">
                  <Ticket size={28} className="mx-auto text-surface-600 mb-2" />
                  <p className="text-sm text-surface-500">Sin datos de cobertura</p>
                </div>
              )}
            </div>

            {/* Events by type */}
            <div className="card">
              <SectionHeader>Eventos GitHub por Tipo</SectionHeader>
              <div className="space-y-2.5">
                {Object.entries(serverStats.github_events.by_type)
                  .sort(([, a], [, b]) => b - a)
                  .slice(0, 5)
                  .map(([eventType, count]) => (
                    <div key={eventType} className="flex items-center justify-between">
                      <span className="text-sm text-surface-300">{eventType}</span>
                      <Badge variant="neutral">{count}</Badge>
                    </div>
                  ))}
                {Object.keys(serverStats.github_events.by_type).length === 0 && (
                  <p className="text-sm text-surface-500 text-center py-2">Sin datos aún</p>
                )}
              </div>
            </div>

            <div className="card">
              <SectionHeader>Eventos Cliente por Estado</SectionHeader>
              <div className="space-y-2.5">
                {Object.entries(serverStats.client_events.by_status)
                  .sort(([, a], [, b]) => b - a)
                  .map(([status, count]) => (
                    <div key={status} className="flex items-center justify-between">
                      <span className="text-sm text-surface-300">{status}</span>
                      <Badge variant={status === 'blocked' ? 'danger' : 'success'}>{count}</Badge>
                    </div>
                  ))}
                {Object.keys(serverStats.client_events.by_status).length === 0 && (
                  <p className="text-sm text-surface-500 text-center py-2">Sin datos aún</p>
                )}
              </div>
            </div>
          </div>

          {/* Commits without ticket / Tickets without commits */}
          <div className="grid grid-cols-2 gap-4">
            <div className="card">
              <div className="flex items-center justify-between mb-3">
                <SectionHeader>Commits sin ticket</SectionHeader>
                {ticketCoverage && (
                  <Badge variant="warning">{ticketCoverage.commits_without_ticket.length}</Badge>
                )}
              </div>
              {commitsWithoutTicket.length > 0 ? (
                <div className="space-y-2">
                  {commitsWithoutTicket.map((item, idx) => {
                    const commitSha = typeof item.commit_sha === 'string' ? item.commit_sha : '-'
                    const shortSha = commitSha !== '-' ? commitSha.slice(0, 7) : '-'
                    const branch = typeof item.branch === 'string' ? item.branch : '-'
                    const user = typeof item.user_login === 'string' ? item.user_login : '-'
                    const createdAt = typeof item.created_at === 'number' ? item.created_at : null
                    return (
                      <div key={`${commitSha}-${idx}`} className="rounded-lg border border-surface-700/50 bg-surface-900/50 p-2.5">
                        <div className="flex items-center gap-2 flex-wrap">
                          <Badge variant="neutral">{shortSha}</Badge>
                          <span className="text-xs text-surface-300 font-mono">{branch}</span>
                          <span className="text-xs text-surface-500">{user}</span>
                        </div>
                        {createdAt && (
                          <div className="text-[11px] text-surface-500 mt-1.5">
                            {new Date(createdAt).toLocaleString()}
                          </div>
                        )}
                      </div>
                    )
                  })}
                  {ticketCoverage && ticketCoverage.commits_without_ticket.length > commitsWithoutTicket.length && (
                    <p className="text-xs text-surface-500">
                      Mostrando {commitsWithoutTicket.length} de {ticketCoverage.commits_without_ticket.length}
                    </p>
                  )}
                </div>
              ) : (
                <p className="text-sm text-surface-500 text-center py-3">Sin commits faltantes</p>
              )}
            </div>

            <div className="card">
              <div className="flex items-center justify-between mb-3">
                <SectionHeader>Tickets sin commits</SectionHeader>
                {ticketCoverage && (
                  <Badge variant="neutral">{ticketCoverage.tickets_without_commits.length}</Badge>
                )}
              </div>
              {(ticketCoverage?.tickets_without_commits ?? []).slice(0, 5).length > 0 ? (
                <div className="space-y-2">
                  {(ticketCoverage?.tickets_without_commits ?? []).slice(0, 5).map((item, idx) => {
                    const ticketId = typeof item.ticket_id === 'string' ? item.ticket_id : '-'
                    const status = typeof item.status === 'string' ? item.status : null
                    return (
                      <div key={`${ticketId}-${idx}`} className="rounded-lg border border-surface-700/50 bg-surface-900/50 p-2.5">
                        <div className="flex items-center gap-2 flex-wrap">
                          <Badge variant="info">{ticketId}</Badge>
                          {status && <Badge variant="warning">{status}</Badge>}
                        </div>
                      </div>
                    )
                  })}
                </div>
              ) : (
                <p className="text-sm text-surface-500 text-center py-3">Sin tickets huérfanos</p>
              )}
            </div>
          </div>

          {/* Recent Commits */}
          <div className="card">
            <div className="flex items-center gap-2 mb-4">
              <GitCommit size={16} className="text-surface-400" />
              <SectionHeader>Commits Recientes</SectionHeader>
            </div>

            {/* Ticket detail panel */}
            {selectedTicketId && (
              <div className="mb-4 rounded-xl border border-brand-500/20 bg-surface-900/80 p-4 animate-scale-in">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <Badge variant="info">{selectedTicketId}</Badge>
                    {selectedTicketDetails && typeof selectedTicketDetails.status === 'string' && (
                      <Badge variant="warning">{selectedTicketDetails.status}</Badge>
                    )}
                    {selectedTicketDetails && typeof selectedTicketDetails.assignee === 'string' && selectedTicketDetails.assignee && (
                      <Badge variant="neutral">{selectedTicketDetails.assignee}</Badge>
                    )}
                    {isSelectedTicketLoading && <Spinner size="sm" className="ml-1" />}
                  </div>
                  <button
                    type="button"
                    className="p-1 rounded-lg text-surface-400 hover:text-white hover:bg-surface-700 transition-colors"
                    onClick={() => setSelectedTicketId(null)}
                  >
                    <X size={14} />
                  </button>
                </div>
                <p className="text-xs text-surface-400 mt-2">{ticketPanelSummaryText}</p>
                {selectedTicketDetails && typeof selectedTicketDetails === 'object' && 'ticket_url' in selectedTicketDetails && typeof selectedTicketDetails.ticket_url === 'string' && selectedTicketDetails.ticket_url && (
                  <a
                    href={selectedTicketDetails.ticket_url}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1 mt-2 text-xs text-brand-400 hover:text-brand-300 transition-colors"
                  >
                    <ExternalLink size={11} />
                    Abrir ticket
                  </a>
                )}
                {selectedTicketDetails && typeof selectedTicketDetails === 'object' && 'related_branches' in selectedTicketDetails && (
                  <div className="mt-3 border-t border-surface-700/50 pt-2">
                    <button
                      type="button"
                      className="flex items-center gap-1 text-xs text-brand-400 hover:text-brand-300 transition-colors"
                      onClick={() => setTicketPanelExpanded((v) => !v)}
                    >
                      {ticketPanelExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                      {ticketPanelExpanded ? 'Ocultar relaciones' : 'Ver relaciones'}
                    </button>
                    {ticketPanelExpanded && (
                      <div className="mt-2 grid grid-cols-1 md:grid-cols-3 gap-3 animate-slide-up">
                        <div>
                          <div className="text-[10px] text-surface-500 uppercase tracking-wider mb-1.5 font-semibold">Branches</div>
                          <div className="flex flex-col gap-1">
                            {Array.isArray((selectedTicketDetails as Record<string, unknown>).related_branches) &&
                            ((selectedTicketDetails as Record<string, unknown>).related_branches as unknown[]).length > 0 ? (
                              ((selectedTicketDetails as Record<string, unknown>).related_branches as unknown[]).slice(0, 8).map((b, idx) => (
                                <code key={`rb-${idx}`} className="text-xs text-surface-300 break-all bg-surface-800 px-1.5 py-0.5 rounded">
                                  {String(b)}
                                </code>
                              ))
                            ) : (
                              <span className="text-xs text-surface-500">-</span>
                            )}
                          </div>
                        </div>
                        <div>
                          <div className="text-[10px] text-surface-500 uppercase tracking-wider mb-1.5 font-semibold">Commits</div>
                          <div className="flex flex-col gap-1">
                            {Array.isArray((selectedTicketDetails as Record<string, unknown>).related_commits) &&
                            ((selectedTicketDetails as Record<string, unknown>).related_commits as unknown[]).length > 0 ? (
                              ((selectedTicketDetails as Record<string, unknown>).related_commits as unknown[]).slice(0, 8).map((c, idx) => (
                                <code key={`rc-${idx}`} className="text-xs text-surface-300 break-all bg-surface-800 px-1.5 py-0.5 rounded">
                                  {String(c)}
                                </code>
                              ))
                            ) : (
                              <span className="text-xs text-surface-500">-</span>
                            )}
                          </div>
                        </div>
                        <div>
                          <div className="text-[10px] text-surface-500 uppercase tracking-wider mb-1.5 font-semibold">PRs</div>
                          <div className="flex flex-col gap-1">
                            {Array.isArray((selectedTicketDetails as Record<string, unknown>).related_prs) &&
                            ((selectedTicketDetails as Record<string, unknown>).related_prs as unknown[]).length > 0 ? (
                              ((selectedTicketDetails as Record<string, unknown>).related_prs as unknown[]).slice(0, 8).map((p, idx) => (
                                <code key={`rp-${idx}`} className="text-xs text-surface-300 break-all bg-surface-800 px-1.5 py-0.5 rounded">
                                  {String(p)}
                                </code>
                              ))
                            ) : (
                              <span className="text-xs text-surface-500">-</span>
                            )}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}

            {/* Commits table */}
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="text-left text-[10px] text-surface-500 uppercase tracking-wider border-b border-surface-700/50">
                    <th className="pb-3 font-semibold">Hora</th>
                    <th className="pb-3 font-semibold">Usuario</th>
                    <th className="pb-3 font-semibold">Detalle</th>
                    <th className="pb-3 font-semibold">Repo</th>
                    <th className="pb-3 font-semibold">Rama</th>
                    <th className="pb-3 font-semibold">Estado</th>
                  </tr>
                </thead>
                <tbody>
                  {dashboardRows.map(({ log, attachedFiles }) => {
                    const isCommit = log.event_type === 'commit'
                    const canExpandFiles = isCommit && attachedFiles.length > 0
                    const isExpanded = !!expandedCommitRows[log.id]
                    const pipelineRun = isCommit ? findPipelineForLog(log) : null
                    const ticketIds = isCommit ? extractTicketIdsFromCommitLog(log) : []

                    return (
                      <Fragment key={log.id}>
                        <tr className="border-b border-surface-700/30 hover:bg-surface-700/20 transition-colors">
                          <td className="py-2.5 text-xs text-surface-400 whitespace-nowrap">
                            {new Date(log.created_at).toLocaleString()}
                          </td>
                          <td className="py-2.5 text-sm text-white font-medium">{log.user_login || '-'}</td>
                          <td className="py-2.5">
                            <div className="space-y-1">
                              <div className="flex items-center gap-1.5 flex-wrap">
                                <Badge variant="neutral">{log.event_type}</Badge>
                                {isCommit && getShortCommitSha(log) && (
                                  <span className="text-[11px] text-surface-500 font-mono bg-surface-800 px-1.5 py-0.5 rounded">
                                    {getShortCommitSha(log)}
                                  </span>
                                )}
                                {pipelineRun && (
                                  <Badge
                                    variant={
                                      pipelineRun.status === 'success'
                                        ? 'success'
                                        : pipelineRun.status === 'failure'
                                          ? 'danger'
                                          : 'warning'
                                    }
                                  >
                                    ci:{pipelineRun.status}
                                  </Badge>
                                )}
                                {ticketIds.slice(0, 2).map((ticketId) => (
                                  <button
                                    key={`${log.id}-${ticketId}`}
                                    type="button"
                                    onClick={() => setSelectedTicketId((prev) => (prev === ticketId ? null : ticketId))}
                                    className="inline-flex"
                                    title={`Ticket ${ticketId}`}
                                  >
                                    <Badge variant="info" className="hover:ring-brand-400/40 transition-all cursor-pointer">
                                      {ticketId}
                                    </Badge>
                                  </button>
                                ))}
                                {ticketIds.length > 2 && (
                                  <Badge variant="neutral">+{ticketIds.length - 2}</Badge>
                                )}
                                {canExpandFiles && (
                                  <button
                                    type="button"
                                    className="flex items-center gap-0.5 text-xs text-brand-400 hover:text-brand-300 transition-colors"
                                    onClick={() =>
                                      setExpandedCommitRows((prev) => ({ ...prev, [log.id]: !prev[log.id] }))
                                    }
                                  >
                                    {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                                    {isExpanded ? 'Ocultar' : `${attachedFiles.length} archivos`}
                                  </button>
                                )}
                              </div>
                              {getLogDetailPreview(log) && (
                                <div
                                  className="text-xs text-surface-400 max-w-64 truncate"
                                  title={getLogDetailPreview(log) ?? undefined}
                                >
                                  {getLogDetailPreview(log)}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="py-2.5 text-xs text-surface-400">{log.repo_name || '-'}</td>
                          <td className="py-2.5 text-xs text-surface-400 font-mono">{log.branch || '-'}</td>
                          <td className="py-2.5">
                            <Badge
                              variant={
                                log.status === 'success' ? 'success' : log.status === 'blocked' ? 'danger' : 'warning'
                              }
                            >
                              {log.status || '-'}
                            </Badge>
                          </td>
                        </tr>
                        {canExpandFiles && isExpanded && (
                          <tr className="border-b border-surface-700/30">
                            <td />
                            <td colSpan={5} className="pb-3 pt-1">
                              <div className="bg-surface-900/50 rounded-lg p-3 border border-surface-700/30 animate-slide-up">
                                <div className="text-[10px] text-surface-500 uppercase tracking-wider font-semibold mb-2">Archivos del commit</div>
                                <div className="flex flex-col gap-1">
                                  {attachedFiles.map((file) => (
                                    <code key={`${log.id}-${file}`} className="text-xs text-surface-300 break-all">
                                      {file}
                                    </code>
                                  ))}
                                </div>
                              </div>
                            </td>
                          </tr>
                        )}
                      </Fragment>
                    )
                  })}
                  {dashboardRows.length === 0 && (
                    <tr>
                      <td colSpan={6} className="py-8 text-center">
                        <GitCommit size={28} className="mx-auto text-surface-600 mb-2" />
                        <p className="text-sm text-surface-500">Sin commits aún</p>
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
