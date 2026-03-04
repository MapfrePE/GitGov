import { useEffect, useRef, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Server } from 'lucide-react'
import { formatTs } from '@/lib/timezone'
import { DashboardHeader } from './DashboardHeader'
import { MetricsGrid } from './MetricsGrid'
import { PipelineHealthWidget } from './PipelineHealthWidget'
import { DailyActivityWidget } from './DailyActivityWidget'
import { TicketCoverageWidget } from './TicketCoverageWidget'
import { EventBreakdownGrid } from './EventBreakdownGrid'
import { RecentCommitsTable } from './RecentCommitsTable'
import { DeveloperAccessPanel } from './DeveloperAccessPanel'
import { ConversationalChatPanel } from './ConversationalChatPanel'
import { MaintenanceOverlay } from './MaintenanceOverlay'
import { Modal } from '@/components/shared/Modal'
import { Badge } from '@/components/shared/Badge'

const DASHBOARD_LOG_LIMIT = 500

export function ServerDashboard() {
  const serverStats = useControlPlaneStore((s) => s.serverStats)
  const dailyActivity = useControlPlaneStore((s) => s.dailyActivity)
  const ticketCoverage = useControlPlaneStore((s) => s.ticketCoverage)
  const userRole = useControlPlaneStore((s) => s.userRole)
  const isConnected = useControlPlaneStore((s) => s.isConnected)
  const connectionStatus = useControlPlaneStore((s) => s.connectionStatus)
  const isRefreshingDashboard = useControlPlaneStore((s) => s.isRefreshingDashboard)
  const refreshForCurrentRole = useControlPlaneStore((s) => s.refreshForCurrentRole)
  const loadLogs = useControlPlaneStore((s) => s.loadLogs)
  const activeDevs7d = useControlPlaneStore((s) => s.activeDevs7d)
  const activeDevs7dUpdatedAt = useControlPlaneStore((s) => s.activeDevs7dUpdatedAt)
  const loadActiveDevs7d = useControlPlaneStore((s) => s.loadActiveDevs7d)
  const displayTimezone = useControlPlaneStore((s) => s.displayTimezone)
  const isChatLoading = useControlPlaneStore((s) => s.isChatLoading)

  const isAdmin = userRole === 'Admin'

  const [autoRefresh, setAutoRefresh] = useState(true)
  const [showActiveDevsModal, setShowActiveDevsModal] = useState(false)
  const isChatLoadingRef = useRef(isChatLoading)

  useEffect(() => {
    isChatLoadingRef.current = isChatLoading
  }, [isChatLoading])

  useEffect(() => {
    if (!isConnected) return

    const runRefresh = () => {
      if (isChatLoadingRef.current) return
      if (userRole === 'Admin') {
        void refreshForCurrentRole()
      } else {
        void loadLogs(DASHBOARD_LOG_LIMIT, 0)
      }
    }

    runRefresh()
    if (!autoRefresh) return

    const interval = setInterval(() => {
      runRefresh()
    }, 30000)
    return () => clearInterval(interval)
  }, [isConnected, autoRefresh, refreshForCurrentRole, loadLogs, userRole])

  /* ── maintenance mode ── */
  if (connectionStatus === 'maintenance') {
    return <MaintenanceOverlay />
  }

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
  const githubPushesToday = serverStats?.github_events.pushes_today ?? 0
  const desktopPushesToday = serverStats?.client_events.desktop_pushes_today ?? 0
  const totalTrackedPushesToday = githubPushesToday + desktopPushesToday
  const pipeline = serverStats?.pipeline
  const pipelineTotal = pipeline?.total_7d ?? 0
  const pipelineSuccessRate = pipelineTotal > 0 ? (((pipeline?.success_7d ?? 0) / pipelineTotal) * 100).toFixed(1) : '0.0'
  const commitsWithoutTicket = (ticketCoverage?.commits_without_ticket ?? []).slice(0, 5)
  const likelyTestActiveDevs = activeDevs7d.filter((d) => d.suspicious_test_data).length
  const activeDevCoverage = serverStats ? `${activeDevs7d.length}/${serverStats.active_devs_week}` : `${activeDevs7d.length}/-`

  return (
    <div className="space-y-3 animate-fade-in">
      <DashboardHeader
        autoRefresh={autoRefresh}
        onAutoRefreshChange={setAutoRefresh}
        onRefresh={() => {
          if (isChatLoading) return
          if (userRole === 'Admin') {
            void refreshForCurrentRole({ forceHeavy: true })
          } else {
            void loadLogs(DASHBOARD_LOG_LIMIT, 0)
          }
        }}
        isRefreshing={isRefreshingDashboard}
      />
      <div className="flex justify-end">
        <span className="text-[9px] text-surface-500 uppercase tracking-widest bg-white/4 px-2 py-0.5 rounded font-medium">TZ: {displayTimezone}</span>
      </div>

      {isAdmin && serverStats && (
        <>
          <MetricsGrid
            totalGithubEvents={serverStats.github_events.total}
            successRate={successRate}
            activeRepos={serverStats.active_repos}
            desktopPushesToday={desktopPushesToday}
            githubPushesToday={githubPushesToday}
            totalTrackedPushesToday={totalTrackedPushesToday}
            blockedToday={serverStats.client_events.blocked_today}
            activeDevsWeek={serverStats.active_devs_week}
            onOpenActiveDevs={() => setShowActiveDevsModal(true)}
          />

          <div className="grid grid-cols-1 xl:grid-cols-3 gap-3">
            <PipelineHealthWidget
              total={pipelineTotal}
              failure={pipeline?.failure_7d ?? 0}
              avgDurationMs={pipeline?.avg_duration_ms_7d ?? 0}
              reposWithFailures={pipeline?.repos_with_failures_7d ?? 0}
              successRate={pipelineSuccessRate}
            />
            <DailyActivityWidget points={dailyActivity} />
            <TicketCoverageWidget />
          </div>

          <EventBreakdownGrid
            githubByType={serverStats.github_events.by_type}
            clientByStatus={serverStats.client_events.by_status}
            commitsWithoutTicket={commitsWithoutTicket}
            ticketsWithoutCommits={(ticketCoverage?.tickets_without_commits ?? []).slice(0, 5)}
            totalCommitsWithoutTicket={ticketCoverage?.commits_without_ticket.length ?? 0}
            totalTicketsWithoutCommits={ticketCoverage?.tickets_without_commits.length ?? 0}
          />

          <RecentCommitsTable />

          <ConversationalChatPanel />

          <Modal
            isOpen={showActiveDevsModal}
            onClose={() => setShowActiveDevsModal(false)}
            title="Detalle: Devs Activos 7d"
            size="xl"
          >
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="text-[11px] text-surface-400">
                  Visibles en muestra: <span className="mono-data text-surface-200">{activeDevCoverage}</span>
                  <span className="ml-2 text-surface-600">(ventana de logs, no forense completa)</span>
                </div>
                <button
                  type="button"
                  onClick={() => void loadActiveDevs7d()}
                  className="text-[10px] text-brand-400 hover:text-brand-300 transition-colors"
                >
                  Actualizar lista
                </button>
              </div>

              <div className="flex items-center gap-2 text-[10px]">
                <Badge variant="neutral">
                  al parecer de test: {likelyTestActiveDevs}
                </Badge>
                {activeDevs7dUpdatedAt && (
                  <span className="text-surface-600">actualizado: {formatTs(activeDevs7dUpdatedAt, displayTimezone)}</span>
                )}
              </div>

              <div className="max-h-[420px] overflow-auto border border-white/6 rounded-lg">
                <table className="w-full">
                  <thead className="sticky top-0 bg-surface-800">
                    <tr className="text-left text-[9px] text-surface-600 uppercase tracking-widest">
                      <th className="py-2 px-3 font-medium">Usuario</th>
                      <th className="py-2 px-3 font-medium">Eventos 7d</th>
                      <th className="py-2 px-3 font-medium">Último evento</th>
                      <th className="py-2 px-3 font-medium">Señal</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/3">
                    {activeDevs7d.map((dev) => (
                      <tr key={dev.user_login} className="hover:bg-white/2">
                        <td className="py-2 px-3 text-[11px] text-surface-200 font-medium">{dev.user_login}</td>
                        <td className="py-2 px-3 text-[11px] text-surface-300 mono-data">{dev.events}</td>
                        <td className="py-2 px-3 text-[10px] text-surface-500">{formatTs(dev.last_seen, displayTimezone)}</td>
                        <td className="py-2 px-3">
                          {dev.suspicious_test_data
                            ? <Badge variant="neutral">aparente test</Badge>
                            : <Badge variant="success">ok</Badge>}
                        </td>
                      </tr>
                    ))}
                    {activeDevs7d.length === 0 && (
                      <tr>
                        <td colSpan={4} className="py-8 text-center text-[11px] text-surface-600">Sin datos en la ventana actual.</td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          </Modal>
        </>
      )}

      {!isAdmin && (
        <>
          <DeveloperAccessPanel />
          <RecentCommitsTable />
        </>
      )}
    </div>
  )
}
