import { useEffect, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Server } from 'lucide-react'
import { DashboardHeader } from './DashboardHeader'
import { MetricsGrid } from './MetricsGrid'
import { PipelineHealthWidget } from './PipelineHealthWidget'
import { TicketCoverageWidget } from './TicketCoverageWidget'
import { EventBreakdownGrid } from './EventBreakdownGrid'
import { RecentCommitsTable } from './RecentCommitsTable'

export function ServerDashboard() {
  const {
    serverStats, ticketCoverage,
    isConnected, isRefreshingDashboard, refreshDashboardData,
  } = useControlPlaneStore()

  const [autoRefresh, setAutoRefresh] = useState(true)

  useEffect(() => {
    if (!isConnected) return
    void refreshDashboardData({ logLimit: 50 })
    if (!autoRefresh) return
    const interval = setInterval(() => { void refreshDashboardData({ logLimit: 50 }) }, 30000)
    return () => clearInterval(interval)
  }, [isConnected, autoRefresh, refreshDashboardData])

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

  return (
    <div className="space-y-3 animate-fade-in">
      <DashboardHeader
        autoRefresh={autoRefresh}
        onAutoRefreshChange={setAutoRefresh}
        onRefresh={() => void refreshDashboardData({ logLimit: 50 })}
        isRefreshing={isRefreshingDashboard}
      />

      {serverStats && (
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
          />

          <div className="grid grid-cols-[3fr_2fr] gap-3">
            <PipelineHealthWidget
              total={pipelineTotal}
              failure={pipeline?.failure_7d ?? 0}
              avgDurationMs={pipeline?.avg_duration_ms_7d ?? 0}
              reposWithFailures={pipeline?.repos_with_failures_7d ?? 0}
              successRate={pipelineSuccessRate}
            />
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
        </>
      )}
    </div>
  )
}
