import { Fragment, useState, useCallback } from 'react'
import { GitCommit, X, ChevronDown, ChevronRight, ChevronLeft, ExternalLink } from 'lucide-react'
import { Badge } from '@/components/shared/Badge'
import { Spinner } from '@/components/shared/Spinner'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import {
  readDetailString, getLogDetailPreview, getShortCommitSha,
  extractTicketIdsFromCommitLog, buildDashboardRows,
  type DashboardRow,
} from './dashboard-helpers'
import { formatTs } from '@/lib/timezone'
import type { CombinedEvent } from '@/lib/types'

interface CommitPipelineRun {
  pipeline_event_id: string
  pipeline_id: string
  job_name: string
  status: string
  duration_ms?: number | null
  triggered_by?: string | null
  ingested_at: number
}

function isLikelySyntheticEvent(log: CombinedEvent): boolean {
  const login = (log.user_login ?? '').trim()
  const syntheticLogin = /^(alias_|erase_ok_|hb_user_|user_[0-9a-f]{6,}|test_?user|golden_?test|smoke|manual-check|victim_)/i.test(login)
  if (syntheticLogin) return true

  const emptyRepoBranch = !log.repo_name && !log.branch
  const syntheticEventType = ['commit', 'attempt_push', 'successful_push', 'heartbeat'].includes(log.event_type)
  return emptyRepoBranch && syntheticEventType
}

export function RecentCommitsTable() {
  const {
    serverLogs, jenkinsCorrelations, ticketCoverage,
    prMergeEvidence,
    jiraTicketDetails, jiraTicketDetailLoading,
    loadJiraTicketDetail,
    logsPage, logsPageSize, setLogsPage,
    displayTimezone,
  } = useControlPlaneStore()

  const [expandedCommitRows, setExpandedCommitRows] = useState<Record<string, boolean>>({})
  const [selectedTicketId, setSelectedTicketId] = useState<string | null>(null)
  const [ticketPanelExpanded, setTicketPanelExpanded] = useState(false)

  const selectTicket = useCallback((ticketId: string | null) => {
    setSelectedTicketId(ticketId)
    setTicketPanelExpanded(false)
    if (ticketId) void loadJiraTicketDetail(ticketId)
  }, [loadJiraTicketDetail])

  const allRows: DashboardRow[] = buildDashboardRows(serverLogs)
  const totalRows = allRows.length
  const totalPages = Math.max(1, Math.ceil(totalRows / logsPageSize))
  const safePage = Math.min(logsPage, totalPages - 1)
  const dashboardRows = allRows.slice(safePage * logsPageSize, (safePage + 1) * logsPageSize)

  const pipelineByCommitSha = new Map(
    jenkinsCorrelations.filter((c) => c.pipeline && c.commit_sha).map((c) => [c.commit_sha.toLowerCase(), c.pipeline!]),
  )
  const prEvidenceByHeadSha = new Map(
    prMergeEvidence
      .filter((entry) => entry.head_sha && entry.approvals_count >= 0)
      .map((entry) => [entry.head_sha!.toLowerCase(), entry]),
  )

  const findPipelineForLog = (log: CombinedEvent): CommitPipelineRun | null => {
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

  const findPrEvidenceForLog = (log: CombinedEvent): { approvals_count: number; pr_number: number } | null => {
    const sha = readDetailString(log, 'commit_sha')
    if (!sha) return null
    const normalized = sha.toLowerCase()
    const exact = prEvidenceByHeadSha.get(normalized)
    if (exact) return { approvals_count: exact.approvals_count, pr_number: exact.pr_number }
    for (const [fullSha, entry] of prEvidenceByHeadSha.entries()) {
      if (fullSha.startsWith(normalized) || normalized.startsWith(fullSha)) {
        return { approvals_count: entry.approvals_count, pr_number: entry.pr_number }
      }
    }
    return null
  }

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

  return (
    <div className="glass-panel p-5">
      <div className="card-header mb-1">
        <GitCommit size={11} strokeWidth={1.5} className="text-surface-400" />
        Commits Recientes
      </div>
      <p className="text-xs text-surface-400 mb-4">Mostrando ventana reciente, no histórico completo</p>

      {/* Ticket detail panel */}
      {selectedTicketId && (
        <TicketDetailPanel
          ticketId={selectedTicketId}
          details={selectedTicketDetails}
          isLoading={isSelectedTicketLoading}
          summaryText={ticketPanelSummaryText}
          expanded={ticketPanelExpanded}
          onToggleExpanded={() => setTicketPanelExpanded((v) => !v)}
          onClose={() => selectTicket(null)}
        />
      )}

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="text-left text-[11px] text-surface-400 uppercase tracking-wide">
              <th className="pb-3 pr-4 font-medium">Hora</th>
              <th className="pb-3 pr-4 font-medium">Usuario</th>
              <th className="pb-3 pr-4 font-medium">Detalle</th>
              <th className="pb-3 pr-4 font-medium">Repo</th>
              <th className="pb-3 pr-4 font-medium">Rama</th>
              <th className="pb-3 pr-4 font-medium">Aprob.</th>
              <th className="pb-3 font-medium">Estado</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-white/3">
            {dashboardRows.map(({ log, attachedFiles }) => {
              const isCommit = log.event_type === 'commit'
              const canExpandFiles = isCommit && attachedFiles.length > 0
              const isExpanded = !!expandedCommitRows[log.id]
              const pipelineRun = isCommit ? findPipelineForLog(log) : null
              const prEvidence = isCommit ? findPrEvidenceForLog(log) : null
              const ticketIds = isCommit ? extractTicketIdsFromCommitLog(log) : []
              const isSynthetic = isLikelySyntheticEvent(log)
              return (
                <Fragment key={log.id}>
                  <tr className="hover:bg-white/1.5 transition-colors">
                    <td className="py-2.5 pr-4 text-xs text-surface-300 whitespace-nowrap mono-data">{formatTs(log.created_at, displayTimezone)}</td>
                    <td className="py-2.5 pr-4 text-sm text-surface-100 font-medium">{log.user_login || '-'}</td>
                    <td className="py-2.5 pr-4">
                      <div className="space-y-1">
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <Badge variant="neutral">{log.event_type}</Badge>
                          {isSynthetic && <Badge variant="neutral">aparente test</Badge>}
                          {isCommit && getShortCommitSha(log) && <code className="text-[11px] text-surface-300 mono-data">{getShortCommitSha(log)}</code>}
                          {pipelineRun && <Badge variant={pipelineRun.status === 'success' ? 'success' : pipelineRun.status === 'failure' ? 'danger' : 'warning'}>ci:{pipelineRun.status}</Badge>}
                          {prEvidence && <Badge variant={prEvidence.approvals_count >= 2 ? 'success' : 'danger'}>PR #{prEvidence.pr_number}</Badge>}
                          {ticketIds.slice(0, 2).map((ticketId) => (
                            <button key={`${log.id}-${ticketId}`} type="button" onClick={() => selectTicket(selectedTicketId === ticketId ? null : ticketId)} className="inline-flex" title={`Ticket ${ticketId}`}>
                              <Badge variant="info" className="hover:ring-brand-400/30 transition-all cursor-pointer">{ticketId}</Badge>
                            </button>
                          ))}
                          {ticketIds.length > 2 && <Badge variant="neutral">+{ticketIds.length - 2}</Badge>}
                          {canExpandFiles && (
                            <button type="button" className="flex items-center gap-0.5 text-xs text-brand-300 hover:text-brand-200 transition-colors" onClick={() => setExpandedCommitRows((prev) => ({ ...prev, [log.id]: !prev[log.id] }))}>
                              {isExpanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                              {isExpanded ? 'Ocultar' : `${attachedFiles.length} archivos`}
                            </button>
                          )}
                        </div>
                        {getLogDetailPreview(log) && <div className="text-xs text-surface-300 max-w-64 truncate" title={getLogDetailPreview(log) ?? undefined}>{getLogDetailPreview(log)}</div>}
                      </div>
                    </td>
                    <td className="py-2.5 pr-4 text-xs text-surface-300">{log.repo_name || '-'}</td>
                    <td className="py-2.5 pr-4 text-xs text-surface-300 mono-data">{log.branch || '-'}</td>
                    <td className="py-2.5 pr-4">
                      {isCommit && prEvidence
                        ? (
                          <Badge variant={prEvidence.approvals_count >= 2 ? 'success' : 'danger'}>
                            {prEvidence.approvals_count >= 2
                              ? `${prEvidence.approvals_count}/2+`
                              : `${prEvidence.approvals_count}/2`}
                          </Badge>
                          )
                        : <span className="text-xs text-surface-500">-</span>}
                    </td>
                    <td className="py-2.5"><Badge variant={log.status === 'success' ? 'success' : log.status === 'blocked' ? 'danger' : 'warning'}>{log.status || '-'}</Badge></td>
                  </tr>
                  {canExpandFiles && isExpanded && (
                    <tr>
                      <td />
                      <td colSpan={6} className="pb-3 pt-1">
                        <div className="pl-3 border-l border-white/6 animate-slide-up">
                          <div className="text-[11px] text-surface-400 uppercase tracking-wide font-medium mb-1.5">Archivos del commit</div>
                          <div className="flex flex-col gap-0.5">
                            {attachedFiles.map((file) => <code key={`${log.id}-${file}`} className="text-xs text-surface-300 break-all mono-data">{file}</code>)}
                          </div>
                        </div>
                      </td>
                    </tr>
                  )}
                </Fragment>
              )
            })}
            {dashboardRows.length === 0 && (
              <tr><td colSpan={7} className="py-12 text-center"><GitCommit size={18} strokeWidth={1.5} className="mx-auto text-surface-700 mb-2" /><p className="text-xs text-surface-400">Sin commits aún</p></td></tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination controls */}
      {totalRows > 0 && (
        <div className="flex items-center justify-between mt-3 pt-3 border-t border-white/4">
          <span className="text-xs text-surface-300">
            Página {safePage + 1} de {totalPages}
            <span className="ml-1 text-surface-500">({totalRows} total)</span>
          </span>
          <div className="flex items-center gap-1">
            <button
              type="button"
              disabled={safePage === 0}
              onClick={() => setLogsPage(safePage - 1)}
              className="flex items-center gap-1 px-3 py-1.5 rounded-md text-sm text-surface-200 bg-white/5 border border-white/10 hover:bg-white/10 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              <ChevronLeft size={13} />
              Prev
            </button>
            <button
              type="button"
              disabled={safePage >= totalPages - 1}
              onClick={() => setLogsPage(safePage + 1)}
              className="flex items-center gap-1 px-3 py-1.5 rounded-md text-sm text-surface-200 bg-white/5 border border-white/10 hover:bg-white/10 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              Next
              <ChevronRight size={13} />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

/* ── Ticket Detail Panel ── */

interface TicketDetailPanelProps {
  ticketId: string
  details: object | null
  isLoading: boolean
  summaryText: string
  expanded: boolean
  onToggleExpanded: () => void
  onClose: () => void
}

function TicketDetailPanel({ ticketId, details, isLoading, summaryText, expanded, onToggleExpanded, onClose }: TicketDetailPanelProps) {
  const detailMap = details && typeof details === 'object' ? (details as Record<string, unknown>) : null
  return (
    <div className="mb-4 rounded-xl bg-white/2 border border-white/6 p-4 animate-scale-in">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 flex-wrap">
          <Badge variant="info">{ticketId}</Badge>
          {detailMap && typeof detailMap.status === 'string' && <Badge variant="warning">{detailMap.status}</Badge>}
          {detailMap && typeof detailMap.assignee === 'string' && detailMap.assignee && <Badge variant="neutral">{detailMap.assignee}</Badge>}
          {isLoading && <Spinner size="sm" className="ml-1" />}
        </div>
        <button type="button" className="p-1 rounded text-surface-600 hover:text-surface-400 transition-colors" onClick={onClose}><X size={13} strokeWidth={1.5} /></button>
      </div>
      <p className="text-xs text-surface-300 mt-2 leading-relaxed">{summaryText}</p>
      {detailMap && typeof detailMap.ticket_url === 'string' && detailMap.ticket_url && (
        <a href={detailMap.ticket_url} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 mt-2 text-xs text-brand-300 hover:text-brand-200 transition-colors"><ExternalLink size={11} />Abrir ticket</a>
      )}
      {detailMap && 'related_branches' in detailMap && (
        <div className="mt-3 border-t border-white/4 pt-2">
          <button type="button" className="flex items-center gap-1 text-xs text-brand-300 hover:text-brand-200 transition-colors" onClick={onToggleExpanded}>
            {expanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
            {expanded ? 'Ocultar relaciones' : 'Ver relaciones'}
          </button>
          {expanded && (
            <div className="mt-2 grid grid-cols-3 gap-3 animate-slide-up">
              {['related_branches', 'related_commits', 'related_prs'].map((field) => {
                const label = field === 'related_branches' ? 'Branches' : field === 'related_commits' ? 'Commits' : 'PRs'
                const items = Array.isArray(detailMap[field]) ? (detailMap[field] as unknown[]).slice(0, 8) : []
                return (
                  <div key={field}>
                    <div className="text-[11px] text-surface-400 uppercase tracking-wide mb-1 font-medium">{label}</div>
                    <div className="flex flex-col gap-0.5">
                      {items.length > 0 ? items.map((b, idx) => <code key={`${field}-${idx}`} className="text-xs text-surface-300 break-all mono-data">{String(b)}</code>) : <span className="text-xs text-surface-500">-</span>}
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
