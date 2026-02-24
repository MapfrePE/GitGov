import { Fragment, useEffect, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Badge } from '@/components/shared/Badge'
import { TrendingUp, Users, AlertTriangle, Activity, Server } from 'lucide-react'
import type { CombinedEvent } from '@/lib/types'

interface StatCardProps {
  icon: React.ReactNode
  label: string
  value: string | number
  color: 'brand' | 'success' | 'warning' | 'danger'
}

const colorClasses = {
  brand: 'bg-brand-500/20 text-brand-400',
  success: 'bg-success-500/20 text-success-400',
  warning: 'bg-warning-500/20 text-warning-400',
  danger: 'bg-danger-500/20 text-danger-400',
}

function StatCard({ icon, label, value, color }: StatCardProps) {
  return (
    <div className="bg-surface-800 rounded-lg p-4 border border-surface-700">
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-2 rounded ${colorClasses[color]}`}>{icon}</div>
        <span className="text-sm text-surface-400">{label}</span>
      </div>
      <p className="text-2xl font-bold text-white">{value}</p>
    </div>
  )
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

    let attachedFiles: string[] = []

    if (log.event_type === 'commit') {
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
    }

    rows.push({ log, attachedFiles })
  }

  return rows
}

export function ServerDashboard() {
  const { serverStats, serverLogs, isConnected, isLoading, loadStats, loadLogs } = useControlPlaneStore()
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [expandedCommitRows, setExpandedCommitRows] = useState<Record<string, boolean>>({})

  useEffect(() => {
    if (isConnected && autoRefresh) {
      loadStats()
      loadLogs(50)
    }
  }, [isConnected, autoRefresh, loadStats, loadLogs])

  useEffect(() => {
    if (!isConnected || !autoRefresh) return
    
    const interval = setInterval(() => {
      loadStats()
      loadLogs(50)
    }, 30000)
    
    return () => clearInterval(interval)
  }, [isConnected, autoRefresh, loadStats, loadLogs])

  if (!isConnected) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-surface-500">
        <Server size={48} className="mb-4" />
        <p>Conecta a un servidor Control Plane para ver el dashboard</p>
      </div>
    )
  }

  const successRate = serverStats
    ? serverStats.github_events.pushes_today + serverStats.client_events.blocked_today > 0
      ? ((serverStats.github_events.pushes_today / (serverStats.github_events.pushes_today + serverStats.client_events.blocked_today)) * 100).toFixed(1)
      : '100.0'
    : '0'
  const dashboardRows = buildDashboardRows(serverLogs).slice(0, 10)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold text-white">Dashboard del Control Plane</h2>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-2 text-sm text-surface-400">
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
              className="rounded border-surface-600"
            />
            Auto-actualizar
          </label>
          <Button variant="ghost" size="sm" onClick={() => { loadStats(); loadLogs(50); }} loading={isLoading}>
            Actualizar
          </Button>
        </div>
      </div>

      {serverStats && (
        <>
          <div className="grid grid-cols-4 gap-4">
            <StatCard
              icon={<TrendingUp size={16} />}
              label="Total Eventos GitHub"
              value={serverStats.github_events.total}
              color="brand"
            />
            <StatCard
              icon={<Activity size={16} />}
              label="Pushes Hoy"
              value={serverStats.github_events.pushes_today}
              color="success"
            />
            <StatCard
              icon={<AlertTriangle size={16} />}
              label="Bloqueados Hoy"
              value={serverStats.client_events.blocked_today}
              color="danger"
            />
            <StatCard
              icon={<Users size={16} />}
              label="Devs Activos (Semana)"
              value={serverStats.active_devs_week}
              color="warning"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="card">
              <h3 className="text-sm font-medium text-white mb-3">Tasa de Éxito</h3>
              <div className="flex items-center gap-4">
                <div className="text-4xl font-bold text-success-400">{successRate}%</div>
                <div className="flex-1">
                  <div className="h-3 bg-surface-700 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-success-500 rounded-full"
                      style={{ width: `${successRate}%` }}
                    />
                  </div>
                </div>
              </div>
            </div>

            <div className="card">
              <h3 className="text-sm font-medium text-white mb-3">Repos Activos</h3>
              <div className="flex items-center gap-4">
                <div className="text-4xl font-bold text-brand-400">{serverStats.active_repos}</div>
                <span className="text-sm text-surface-400">últimos 7 días</span>
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="card">
              <h3 className="text-sm font-medium text-white mb-3">Eventos GitHub por Tipo</h3>
              <div className="space-y-2">
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
                  <p className="text-sm text-surface-500">Sin datos aún</p>
                )}
              </div>
            </div>

            <div className="card">
              <h3 className="text-sm font-medium text-white mb-3">Eventos Cliente por Estado</h3>
              <div className="space-y-2">
                {Object.entries(serverStats.client_events.by_status)
                  .sort(([, a], [, b]) => b - a)
                  .map(([status, count]) => (
                    <div key={status} className="flex items-center justify-between">
                      <span className="text-sm text-surface-300">{status}</span>
                      <Badge variant={status === 'blocked' ? 'danger' : 'success'}>{count}</Badge>
                    </div>
                  ))}
                {Object.keys(serverStats.client_events.by_status).length === 0 && (
                  <p className="text-sm text-surface-500">Sin datos aún</p>
                )}
              </div>
            </div>
          </div>

          <div className="card">
            <h3 className="text-sm font-medium text-white mb-3">Eventos Recientes</h3>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="text-left text-xs text-surface-500 border-b border-surface-700">
                    <th className="pb-2">Hora</th>
                    <th className="pb-2">Usuario</th>
                    <th className="pb-2">Tipo</th>
                    <th className="pb-2">Repo</th>
                    <th className="pb-2">Rama</th>
                    <th className="pb-2">Estado</th>
                  </tr>
                </thead>
                <tbody>
                  {dashboardRows.map(({ log, attachedFiles }) => {
                    const isCommit = log.event_type === 'commit'
                    const canExpandFiles = isCommit && attachedFiles.length > 0
                    const isExpanded = !!expandedCommitRows[log.id]

                    return (
                      <Fragment key={log.id}>
                        <tr key={log.id} className="border-b border-surface-700/50">
                          <td className="py-2 text-sm text-surface-400">
                            {new Date(log.created_at).toLocaleString()}
                          </td>
                          <td className="py-2 text-sm text-white">{log.user_login || '-'}</td>
                          <td className="py-2">
                            <div className="space-y-1">
                              <div className="flex items-center gap-2 flex-wrap">
                                <Badge variant="neutral">{log.event_type}</Badge>
                                {isCommit && getShortCommitSha(log) && (
                                  <span className="text-xs text-surface-500 font-mono">
                                    {getShortCommitSha(log)}
                                  </span>
                                )}
                                {canExpandFiles && (
                                  <button
                                    type="button"
                                    className="text-xs text-brand-400 hover:text-brand-300 underline"
                                    onClick={() =>
                                      setExpandedCommitRows((prev) => ({ ...prev, [log.id]: !prev[log.id] }))
                                    }
                                  >
                                    {isExpanded ? 'Ocultar archivos' : `Ver archivos (${attachedFiles.length})`}
                                  </button>
                                )}
                              </div>
                              {getLogDetailPreview(log) && (
                                <div
                                  className="text-xs text-surface-400 max-w-56 truncate"
                                  title={getLogDetailPreview(log) ?? undefined}
                                >
                                  {getLogDetailPreview(log)}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="py-2 text-sm text-surface-300">{log.repo_name || '-'}</td>
                          <td className="py-2 text-sm text-surface-300 font-mono">{log.branch || '-'}</td>
                          <td className="py-2">
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
                          <tr className="border-b border-surface-700/50">
                            <td />
                            <td colSpan={5} className="pb-3">
                              <div className="bg-surface-900 rounded-md p-2">
                                <div className="text-xs text-surface-400 mb-2">Archivos del commit</div>
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
                      <td colSpan={6} className="py-4 text-center text-surface-500">
                        Sin eventos aún
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
