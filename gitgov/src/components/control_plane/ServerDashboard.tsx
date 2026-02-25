import { Fragment, useEffect, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Badge } from '@/components/shared/Badge'
import { Spinner } from '@/components/shared/Spinner'
import { TrendingUp, Users, AlertTriangle, Activity, Server, GitCommit, Workflow, Ticket, X, ChevronDown, ChevronRight, ExternalLink } from 'lucide-react'
import type { CombinedEvent } from '@/lib/types'

/* ── helpers (unchanged logic) ── */

function readDetailString(log: CombinedEvent, key: string): string | null {
  const value = log.details?.[key]
  if (typeof value === 'string' && value.trim().length > 0) return value
  const metadata = log.details && typeof log.details === 'object' ? (log.details['metadata'] as Record<string, unknown> | undefined) : undefined
  const nested = metadata?.[key]
  if (typeof nested === 'string' && nested.trim().length > 0) return nested
  const legacyDetails = log.details && typeof log.details === 'object' ? (log.details['legacy_details'] as Record<string, unknown> | undefined) : undefined
  const legacyMetadata = legacyDetails && typeof legacyDetails === 'object' ? (legacyDetails['metadata'] as Record<string, unknown> | undefined) : undefined
  const nestedLegacy = legacyMetadata?.[key]
  return typeof nestedLegacy === 'string' && nestedLegacy.trim().length > 0 ? nestedLegacy : null
}

function getLogDetailPreview(log: CombinedEvent): string | null {
  if (log.event_type === 'commit') return readDetailString(log, 'commit_message')
  if (log.status === 'failed' || log.status === 'blocked') return readDetailString(log, 'reason')
  return null
}

function getShortCommitSha(log: CombinedEvent): string | null {
  const sha = readDetailString(log, 'commit_sha')
  return sha ? sha.slice(0, 7) : null
}

function extractTicketIdsFromCommitLog(log: CombinedEvent): string[] {
  const values = [readDetailString(log, 'commit_message'), log.branch ?? null].filter((v): v is string => typeof v === 'string' && v.trim().length > 0)
  const regex = /\b([A-Z][A-Z0-9]{1,15}-\d{1,9})\b/g
  const result: string[] = []
  const seen = new Set<string>()
  for (const value of values) {
    let match: RegExpExecArray | null
    regex.lastIndex = 0
    while ((match = regex.exec(value)) !== null) {
      const ticket = match[1].toUpperCase()
      if (!seen.has(ticket)) { seen.add(ticket); result.push(ticket) }
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
  if (Array.isArray(direct)) return direct.filter((v): v is string => typeof v === 'string')
  return []
}

interface DashboardRow { log: CombinedEvent; attachedFiles: string[] }

function buildDashboardRows(logs: CombinedEvent[]): DashboardRow[] {
  const rows: DashboardRow[] = []
  const consumedStageFileIds = new Set<string>()
  for (let i = 0; i < logs.length; i++) {
    const log = logs[i]
    if (log.event_type === 'stage_files' || log.event_type !== 'commit') continue
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
      if (files.length > 0) { attachedFiles = files; consumedStageFileIds.add(candidate.id) }
      break
    }
    rows.push({ log, attachedFiles })
  }
  return rows
}

/* ── small inline progress bar ── */
function Bar({ value, color = 'brand' }: { value: number; color?: 'brand' | 'success' }) {
  const bg = color === 'success' ? 'bg-success-500/70' : 'bg-brand-500/70'
  return (
    <div className="h-1 bg-white/[0.04] rounded-full overflow-hidden">
      <div className={`h-full ${bg} rounded-full transition-all duration-700`} style={{ width: `${Math.min(100, value)}%` }} />
    </div>
  )
}

/* ══════════════════════════════════════════════════════════════ */

export function ServerDashboard() {
  const {
    serverStats, serverLogs, jenkinsCorrelations, ticketCoverage,
    jiraCoverageFilters, jiraTicketDetails, jiraTicketDetailLoading,
    isConnected, isRefreshingDashboard, refreshDashboardData, loadLogs,
    applyTicketCoverageFilters, correlateJiraTickets, loadJiraTicketDetail,
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
    const interval = setInterval(() => { void refreshDashboardData({ logLimit: 50 }) }, 30000)
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

  /* ── not connected ── */
  if (!isConnected) {
    return (
      <div className="flex flex-col items-center justify-center h-64 animate-fade-in">
        <Server size={32} strokeWidth={1.5} className="text-surface-700 mb-3" />
        <p className="text-xs font-medium text-surface-400">Conecta al Control Plane</p>
        <p className="text-[10px] text-surface-600 mt-1">Configura la conexión para ver el dashboard</p>
      </div>
    )
  }

  /* ── derived data ── */
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
    ? (jiraTicketDetails[selectedTicketId] ?? (ticketCoverage?.tickets_without_commits ?? []).find((t) => typeof t.ticket_id === 'string' && t.ticket_id === selectedTicketId) ?? null)
    : null
  const isSelectedTicketLoading = selectedTicketId ? !!jiraTicketDetailLoading[selectedTicketId] : false
  const ticketPanelSummaryText = isSelectedTicketLoading
    ? 'Cargando detalle de Jira...'
    : selectedTicketDetails && typeof selectedTicketDetails === 'object' && 'title' in selectedTicketDetails && typeof selectedTicketDetails.title === 'string' && selectedTicketDetails.title
      ? selectedTicketDetails.title
      : selectedTicketDetails
        ? 'Detalle parcial desde coverage.'
        : 'Ticket detectado. Ingiere Jira y ejecuta correlación para más detalle.'
  const dashboardRows = buildDashboardRows(serverLogs).slice(0, 10)
  const pipelineByCommitSha = new Map(
    jenkinsCorrelations.filter((c) => c.pipeline && c.commit_sha).map((c) => [c.commit_sha.toLowerCase(), c.pipeline!]),
  )
  const findPipelineForLog = (log: CombinedEvent) => {
    const sha = readDetailString(log, 'commit_sha')
    if (!sha) return null
    const normalized = sha.toLowerCase()
    const exact = pipelineByCommitSha.get(normalized)
    if (exact) return exact
    for (const [fullSha, p] of pipelineByCommitSha.entries()) {
      if (fullSha.startsWith(normalized) || normalized.startsWith(fullSha)) return p
    }
    return null
  }

  return (
    <div className="space-y-3 animate-fade-in">

      {/* ── Top bar: title + controls ── */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-white tracking-tight">Dashboard</h2>
          <p className="text-[10px] text-surface-600">Control Plane overview</p>
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-1.5 text-[10px] text-surface-500 cursor-pointer select-none">
            <input type="checkbox" checked={autoRefresh} onChange={(e) => setAutoRefresh(e.target.checked)} className="rounded border-surface-700 bg-transparent text-brand-500 focus:ring-brand-500/20 w-3 h-3" />
            Auto-refresh
          </label>
          <Button variant="ghost" size="sm" onClick={() => void refreshDashboardData({ logLimit: 50 })} loading={isRefreshingDashboard}>
            Actualizar
          </Button>
        </div>
      </div>

      {serverStats && (
        <>
          {/* ════════ BENTO GRID — Row 1: Hero + 3 compact ════════ */}
          <div className="grid grid-cols-4 grid-rows-[auto_auto] gap-3">
            {/* Hero: col-span-2, row-span-2 */}
            <div className="glass-panel col-span-2 row-span-2 p-6 flex flex-col justify-between" style={{ '--stagger': 0 } as React.CSSProperties}>
              <div className="flex items-center gap-2">
                <TrendingUp size={14} strokeWidth={1.5} className="text-brand-400" />
                <span className="card-header">Total Eventos GitHub</span>
              </div>
              <div className="mt-4">
                <span className="text-6xl font-bold text-white tracking-tighter mono-data leading-none">{serverStats.github_events.total}</span>
              </div>
              <div className="mt-6 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-[10px] text-surface-500 uppercase tracking-widest">Tasa éxito</span>
                  <span className="text-sm text-success-400 font-semibold mono-data">{successRate}%</span>
                </div>
                <Bar value={parseFloat(successRate)} color="success" />
                <div className="flex items-center gap-4 pt-1">
                  <span className="text-[10px] text-surface-500"><span className="text-surface-300 mono-data">{serverStats.active_repos}</span> repos activos</span>
                </div>
              </div>
            </div>

            {/* Pushes */}
            <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 1 } as React.CSSProperties}>
              <div className="flex items-center gap-1.5">
                <Activity size={12} strokeWidth={1.5} className="text-success-400" />
                <span className="card-header">Pushes Hoy</span>
              </div>
              <span className="text-4xl font-bold text-white tracking-tighter mono-data mt-auto leading-none">{serverStats.github_events.pushes_today}</span>
            </div>

            {/* Blocked */}
            <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 2 } as React.CSSProperties}>
              <div className="flex items-center gap-1.5">
                <AlertTriangle size={12} strokeWidth={1.5} className="text-danger-400" />
                <span className="card-header">Bloqueados</span>
              </div>
              <span className="text-4xl font-bold text-white tracking-tighter mono-data mt-auto leading-none">{serverStats.client_events.blocked_today}</span>
            </div>

            {/* Devs */}
            <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 3 } as React.CSSProperties}>
              <div className="flex items-center gap-1.5">
                <Users size={12} strokeWidth={1.5} className="text-warning-400" />
                <span className="card-header">Devs Activos 7d</span>
              </div>
              <span className="text-4xl font-bold text-white tracking-tighter mono-data mt-auto leading-none">{serverStats.active_devs_week}</span>
            </div>

            {/* Success rate compact */}
            <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 4 } as React.CSSProperties}>
              <span className="card-header">Repos Activos</span>
              <span className="text-4xl font-bold text-white tracking-tighter mono-data mt-auto leading-none">{serverStats.active_repos}</span>
              <span className="text-[10px] text-surface-600 mt-1">últimos 7 días</span>
            </div>
          </div>

          {/* ════════ Row 2: Pipeline (wide) + Ticket Coverage ════════ */}
          <div className="grid grid-cols-[3fr_2fr] gap-3">
            {/* Pipeline Health */}
            <div className="glass-panel p-5">
              <div className="card-header mb-4">
                <Workflow size={11} strokeWidth={1.5} className="text-surface-400" />
                Pipeline Health (7d)
              </div>
              {pipelineTotal > 0 ? (
                <div className="space-y-3">
                  <div className="flex items-baseline gap-3">
                    <span className="text-4xl font-bold text-white tracking-tighter mono-data leading-none">{pipelineSuccessRate}%</span>
                    <span className="text-[10px] text-surface-500 uppercase tracking-widest">success rate</span>
                  </div>
                  <Bar value={parseFloat(pipelineSuccessRate)} color="success" />
                  <div className="grid grid-cols-2 gap-x-8 gap-y-2 pt-2">
                    {[
                      ['Pipelines', pipelineTotal, ''],
                      ['Failures', pipeline?.failure_7d ?? 0, 'text-danger-400'],
                      ['Avg duration', formatDurationMs(pipeline?.avg_duration_ms_7d), ''],
                      ['Repos w/ failures', pipeline?.repos_with_failures_7d ?? 0, ''],
                    ].map(([label, val, cls]) => (
                      <div key={String(label)} className="flex items-center justify-between text-[11px]">
                        <span className="text-surface-500">{label}</span>
                        <span className={`mono-data font-medium ${cls || 'text-surface-300'}`}>{val}</span>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="py-10 text-center">
                  <Workflow size={20} strokeWidth={1.5} className="mx-auto text-surface-700 mb-2" />
                  <p className="text-[11px] text-surface-600">Sin datos de pipelines</p>
                </div>
              )}
            </div>

            {/* Ticket Coverage */}
            <div className="glass-panel p-5 flex flex-col">
              <div className="flex items-center justify-between mb-3">
                <div className="card-header">
                  <Ticket size={11} strokeWidth={1.5} className="text-surface-400" />
                  Ticket Coverage
                </div>
                <Button variant="ghost" size="sm" loading={isCorrelatingJira} onClick={async () => {
                  setIsCorrelatingJira(true)
                  try { await correlateJiraTickets({ hours: jiraCoverageFilters.hours, limit: 500, repo_full_name: jiraCoverageFilters.repo_full_name.trim() || undefined }); await loadLogs(50) } finally { setIsCorrelatingJira(false) }
                }}>
                  Correlacionar
                </Button>
              </div>
              {/* Filters row */}
              <div className="flex gap-1.5 mb-2">
                <input value={ticketRepoFilter} onChange={(e) => setTicketRepoFilter(e.target.value)} placeholder="repo" className="flex-1 min-w-0 rounded-lg bg-white/[0.03] border border-white/[0.06] px-2 py-1.5 text-[10px] text-white placeholder:text-surface-600 focus:border-brand-500/40 focus:outline-none transition-colors" />
                <input value={ticketBranchFilter} onChange={(e) => setTicketBranchFilter(e.target.value)} placeholder="rama" className="flex-1 min-w-0 rounded-lg bg-white/[0.03] border border-white/[0.06] px-2 py-1.5 text-[10px] text-white placeholder:text-surface-600 focus:border-brand-500/40 focus:outline-none transition-colors" />
                <select value={ticketHours} onChange={(e) => setTicketHours(Number(e.target.value))} className="w-14 shrink-0 rounded-lg bg-white/[0.03] border border-white/[0.06] px-1 py-1.5 text-[10px] text-white focus:border-brand-500/40 focus:outline-none transition-colors">
                  <option value={24}>24h</option>
                  <option value={72}>72h</option>
                  <option value={168}>7d</option>
                  <option value={720}>30d</option>
                </select>
              </div>
              <div className="flex items-center justify-end gap-1.5 mb-3">
                <Button variant="ghost" size="sm" onClick={() => { setTicketHours(72); setTicketRepoFilter(''); setTicketBranchFilter('') }}>Limpiar</Button>
                <Button variant="secondary" size="sm" onClick={() => applyTicketCoverageFilters({ hours: ticketHours, repo_full_name: ticketRepoFilter.trim(), branch: ticketBranchFilter.trim() })}>Aplicar</Button>
              </div>
              {/* Coverage data */}
              {ticketCoverage && ticketCoverage.total_commits > 0 ? (
                <div className="space-y-3 mt-auto">
                  <div className="flex items-baseline gap-2">
                    <span className="text-3xl font-bold text-white tracking-tighter mono-data leading-none">{ticketCoverage.coverage_percentage.toFixed(1)}%</span>
                    <span className="text-[10px] text-surface-500">cobertura</span>
                  </div>
                  <Bar value={ticketCoverage.coverage_percentage} />
                  <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-[10px]">
                    <div className="flex justify-between"><span className="text-surface-500">Commits</span><span className="text-surface-300 mono-data">{ticketCoverage.total_commits}</span></div>
                    <div className="flex justify-between"><span className="text-surface-500">Con ticket</span><span className="text-success-400 mono-data">{ticketCoverage.commits_with_ticket}</span></div>
                    <div className="flex justify-between"><span className="text-surface-500">Sin ticket</span><span className="text-warning-400 mono-data">{ticketCoverage.commits_without_ticket.length}</span></div>
                    <div className="flex justify-between"><span className="text-surface-500">Huérfanos</span><span className="text-surface-300 mono-data">{ticketCoverage.tickets_without_commits.length}</span></div>
                  </div>
                </div>
              ) : (
                <div className="py-8 text-center mt-auto">
                  <Ticket size={18} strokeWidth={1.5} className="mx-auto text-surface-700 mb-2" />
                  <p className="text-[10px] text-surface-600">Sin datos de cobertura</p>
                </div>
              )}
            </div>
          </div>

          {/* ════════ Row 3: 4-col events breakdown ════════ */}
          <div className="grid grid-cols-4 gap-3">
            {/* GitHub events by type */}
            <div className="glass-panel p-4">
              <div className="card-header mb-3">GitHub por Tipo</div>
              <div className="divide-y divide-white/[0.04]">
                {Object.entries(serverStats.github_events.by_type).sort(([, a], [, b]) => b - a).slice(0, 5).map(([eventType, count]) => (
                  <div key={eventType} className="flex items-center justify-between py-2 first:pt-0 last:pb-0">
                    <span className="text-[10px] text-surface-400">{eventType}</span>
                    <span className="text-[10px] text-surface-300 mono-data font-medium">{count}</span>
                  </div>
                ))}
                {Object.keys(serverStats.github_events.by_type).length === 0 && <p className="text-[10px] text-surface-600 text-center py-3">Sin datos</p>}
              </div>
            </div>

            {/* Client events by status */}
            <div className="glass-panel p-4">
              <div className="card-header mb-3">Cliente por Estado</div>
              <div className="divide-y divide-white/[0.04]">
                {Object.entries(serverStats.client_events.by_status).sort(([, a], [, b]) => b - a).map(([status, count]) => (
                  <div key={status} className="flex items-center justify-between py-2 first:pt-0 last:pb-0">
                    <span className="text-[10px] text-surface-400 uppercase tracking-wide">{status}</span>
                    <Badge variant={status === 'blocked' ? 'danger' : 'success'}>{count}</Badge>
                  </div>
                ))}
                {Object.keys(serverStats.client_events.by_status).length === 0 && <p className="text-[10px] text-surface-600 text-center py-3">Sin datos</p>}
              </div>
            </div>

            {/* Commits without ticket */}
            <div className="glass-panel p-4">
              <div className="flex items-center justify-between mb-3">
                <div className="card-header">Sin ticket</div>
                {ticketCoverage && <Badge variant="warning">{ticketCoverage.commits_without_ticket.length}</Badge>}
              </div>
              {commitsWithoutTicket.length > 0 ? (
                <div className="divide-y divide-white/[0.04]">
                  {commitsWithoutTicket.map((item, idx) => {
                    const sha = typeof item.commit_sha === 'string' ? item.commit_sha.slice(0, 7) : '-'
                    const branch = typeof item.branch === 'string' ? item.branch : '-'
                    return (
                      <div key={`${sha}-${idx}`} className="py-2 first:pt-0">
                        <div className="flex items-center gap-1.5">
                          <code className="text-[9px] text-surface-400 mono-data">{sha}</code>
                          <span className="text-[9px] text-surface-600 mono-data truncate">{branch}</span>
                        </div>
                      </div>
                    )
                  })}
                </div>
              ) : (
                <p className="text-[10px] text-surface-600 text-center py-3">Sin commits faltantes</p>
              )}
            </div>

            {/* Tickets without commits */}
            <div className="glass-panel p-4">
              <div className="flex items-center justify-between mb-3">
                <div className="card-header">Tickets huérfanos</div>
                {ticketCoverage && <Badge variant="neutral">{ticketCoverage.tickets_without_commits.length}</Badge>}
              </div>
              {(ticketCoverage?.tickets_without_commits ?? []).slice(0, 5).length > 0 ? (
                <div className="divide-y divide-white/[0.04]">
                  {(ticketCoverage?.tickets_without_commits ?? []).slice(0, 5).map((item, idx) => {
                    const ticketId = typeof item.ticket_id === 'string' ? item.ticket_id : '-'
                    const status = typeof item.status === 'string' ? item.status : null
                    return (
                      <div key={`${ticketId}-${idx}`} className="flex items-center gap-1.5 py-2 first:pt-0">
                        <Badge variant="info">{ticketId}</Badge>
                        {status && <Badge variant="warning">{status}</Badge>}
                      </div>
                    )
                  })}
                </div>
              ) : (
                <p className="text-[10px] text-surface-600 text-center py-3">Sin tickets huérfanos</p>
              )}
            </div>
          </div>

          {/* ════════ Row 4: Recent Commits — full width ════════ */}
          <div className="glass-panel p-5">
            <div className="card-header mb-4">
              <GitCommit size={11} strokeWidth={1.5} className="text-surface-400" />
              Commits Recientes
            </div>

            {/* Ticket detail panel */}
            {selectedTicketId && (
              <div className="mb-4 rounded-xl bg-white/[0.02] border border-white/[0.06] p-4 animate-scale-in">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <Badge variant="info">{selectedTicketId}</Badge>
                    {selectedTicketDetails && typeof selectedTicketDetails.status === 'string' && <Badge variant="warning">{selectedTicketDetails.status}</Badge>}
                    {selectedTicketDetails && typeof selectedTicketDetails.assignee === 'string' && selectedTicketDetails.assignee && <Badge variant="neutral">{selectedTicketDetails.assignee}</Badge>}
                    {isSelectedTicketLoading && <Spinner size="sm" className="ml-1" />}
                  </div>
                  <button type="button" className="p-1 rounded text-surface-600 hover:text-surface-400 transition-colors" onClick={() => setSelectedTicketId(null)}><X size={13} strokeWidth={1.5} /></button>
                </div>
                <p className="text-[10px] text-surface-400 mt-2 leading-relaxed">{ticketPanelSummaryText}</p>
                {selectedTicketDetails && typeof selectedTicketDetails === 'object' && 'ticket_url' in selectedTicketDetails && typeof selectedTicketDetails.ticket_url === 'string' && selectedTicketDetails.ticket_url && (
                  <a href={selectedTicketDetails.ticket_url} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 mt-2 text-[10px] text-brand-400 hover:text-brand-300 transition-colors"><ExternalLink size={9} />Abrir ticket</a>
                )}
                {selectedTicketDetails && typeof selectedTicketDetails === 'object' && 'related_branches' in selectedTicketDetails && (
                  <div className="mt-3 border-t border-white/[0.04] pt-2">
                    <button type="button" className="flex items-center gap-1 text-[10px] text-brand-400 hover:text-brand-300 transition-colors" onClick={() => setTicketPanelExpanded((v) => !v)}>
                      {ticketPanelExpanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                      {ticketPanelExpanded ? 'Ocultar relaciones' : 'Ver relaciones'}
                    </button>
                    {ticketPanelExpanded && (
                      <div className="mt-2 grid grid-cols-3 gap-3 animate-slide-up">
                        {['related_branches', 'related_commits', 'related_prs'].map((field) => {
                          const label = field === 'related_branches' ? 'Branches' : field === 'related_commits' ? 'Commits' : 'PRs'
                          const items = Array.isArray((selectedTicketDetails as Record<string, unknown>)[field]) ? ((selectedTicketDetails as Record<string, unknown>)[field] as unknown[]).slice(0, 8) : []
                          return (
                            <div key={field}>
                              <div className="text-[9px] text-surface-600 uppercase tracking-widest mb-1 font-medium">{label}</div>
                              <div className="flex flex-col gap-0.5">
                                {items.length > 0 ? items.map((b, idx) => <code key={`${field}-${idx}`} className="text-[9px] text-surface-500 break-all mono-data">{String(b)}</code>) : <span className="text-[9px] text-surface-700">-</span>}
                              </div>
                            </div>
                          )
                        })}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}

            {/* Table */}
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="text-left text-[9px] text-surface-600 uppercase tracking-widest">
                    <th className="pb-3 pr-4 font-medium">Hora</th>
                    <th className="pb-3 pr-4 font-medium">Usuario</th>
                    <th className="pb-3 pr-4 font-medium">Detalle</th>
                    <th className="pb-3 pr-4 font-medium">Repo</th>
                    <th className="pb-3 pr-4 font-medium">Rama</th>
                    <th className="pb-3 font-medium">Estado</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-white/[0.03]">
                  {dashboardRows.map(({ log, attachedFiles }) => {
                    const isCommit = log.event_type === 'commit'
                    const canExpandFiles = isCommit && attachedFiles.length > 0
                    const isExpanded = !!expandedCommitRows[log.id]
                    const pipelineRun = isCommit ? findPipelineForLog(log) : null
                    const ticketIds = isCommit ? extractTicketIdsFromCommitLog(log) : []
                    return (
                      <Fragment key={log.id}>
                        <tr className="hover:bg-white/[0.015] transition-colors">
                          <td className="py-2.5 pr-4 text-[10px] text-surface-500 whitespace-nowrap mono-data">{new Date(log.created_at).toLocaleString()}</td>
                          <td className="py-2.5 pr-4 text-[11px] text-surface-200 font-medium">{log.user_login || '-'}</td>
                          <td className="py-2.5 pr-4">
                            <div className="space-y-1">
                              <div className="flex items-center gap-1.5 flex-wrap">
                                <Badge variant="neutral">{log.event_type}</Badge>
                                {isCommit && getShortCommitSha(log) && <code className="text-[9px] text-surface-500 mono-data">{getShortCommitSha(log)}</code>}
                                {pipelineRun && <Badge variant={pipelineRun.status === 'success' ? 'success' : pipelineRun.status === 'failure' ? 'danger' : 'warning'}>ci:{pipelineRun.status}</Badge>}
                                {ticketIds.slice(0, 2).map((ticketId) => (
                                  <button key={`${log.id}-${ticketId}`} type="button" onClick={() => setSelectedTicketId((prev) => (prev === ticketId ? null : ticketId))} className="inline-flex" title={`Ticket ${ticketId}`}>
                                    <Badge variant="info" className="hover:ring-brand-400/30 transition-all cursor-pointer">{ticketId}</Badge>
                                  </button>
                                ))}
                                {ticketIds.length > 2 && <Badge variant="neutral">+{ticketIds.length - 2}</Badge>}
                                {canExpandFiles && (
                                  <button type="button" className="flex items-center gap-0.5 text-[10px] text-brand-400 hover:text-brand-300 transition-colors" onClick={() => setExpandedCommitRows((prev) => ({ ...prev, [log.id]: !prev[log.id] }))}>
                                    {isExpanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                                    {isExpanded ? 'Ocultar' : `${attachedFiles.length} archivos`}
                                  </button>
                                )}
                              </div>
                              {getLogDetailPreview(log) && <div className="text-[10px] text-surface-500 max-w-64 truncate" title={getLogDetailPreview(log) ?? undefined}>{getLogDetailPreview(log)}</div>}
                            </div>
                          </td>
                          <td className="py-2.5 pr-4 text-[10px] text-surface-500">{log.repo_name || '-'}</td>
                          <td className="py-2.5 pr-4 text-[10px] text-surface-500 mono-data">{log.branch || '-'}</td>
                          <td className="py-2.5"><Badge variant={log.status === 'success' ? 'success' : log.status === 'blocked' ? 'danger' : 'warning'}>{log.status || '-'}</Badge></td>
                        </tr>
                        {canExpandFiles && isExpanded && (
                          <tr>
                            <td />
                            <td colSpan={5} className="pb-3 pt-1">
                              <div className="pl-3 border-l border-white/[0.06] animate-slide-up">
                                <div className="text-[9px] text-surface-600 uppercase tracking-widest font-medium mb-1.5">Archivos del commit</div>
                                <div className="flex flex-col gap-0.5">
                                  {attachedFiles.map((file) => <code key={`${log.id}-${file}`} className="text-[10px] text-surface-500 break-all mono-data">{file}</code>)}
                                </div>
                              </div>
                            </td>
                          </tr>
                        )}
                      </Fragment>
                    )
                  })}
                  {dashboardRows.length === 0 && (
                    <tr><td colSpan={6} className="py-12 text-center"><GitCommit size={18} strokeWidth={1.5} className="mx-auto text-surface-700 mb-2" /><p className="text-[10px] text-surface-600">Sin commits aún</p></td></tr>
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
