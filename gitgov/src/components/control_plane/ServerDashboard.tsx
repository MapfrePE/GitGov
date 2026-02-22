import { useEffect, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Badge } from '@/components/shared/Badge'
import { TrendingUp, Users, AlertTriangle, Activity, GitBranch, Server } from 'lucide-react'

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

export function ServerDashboard() {
  const { serverStats, serverLogs, isConnected, isLoading, loadStats, loadLogs } = useControlPlaneStore()
  const [autoRefresh, setAutoRefresh] = useState(true)

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
    }, 30000)
    
    return () => clearInterval(interval)
  }, [isConnected, autoRefresh, loadStats])

  if (!isConnected) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-surface-500">
        <Server size={48} className="mb-4" />
        <p>Conecta a un servidor Control Plane para ver el dashboard</p>
      </div>
    )
  }

  const successRate = serverStats
    ? serverStats.pushes_today + serverStats.blocked_today > 0
      ? ((serverStats.pushes_today / (serverStats.pushes_today + serverStats.blocked_today)) * 100).toFixed(1)
      : '100.0'
    : '0'

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
              label="Total Eventos"
              value={serverStats.total_events}
              color="brand"
            />
            <StatCard
              icon={<Activity size={16} />}
              label="Pushes Hoy"
              value={serverStats.pushes_today}
              color="success"
            />
            <StatCard
              icon={<AlertTriangle size={16} />}
              label="Bloqueados Hoy"
              value={serverStats.blocked_today}
              color="danger"
            />
            <StatCard
              icon={<Users size={16} />}
              label="Devs Activos (Semana)"
              value={serverStats.active_devs_this_week}
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
              <h3 className="text-sm font-medium text-white mb-3">Repositorios Top</h3>
              <div className="space-y-2">
                {Object.entries(serverStats.events_by_repo)
                  .sort(([, a], [, b]) => b - a)
                  .slice(0, 5)
                  .map(([repo, count]) => (
                    <div key={repo} className="flex items-center justify-between">
                      <span className="text-sm text-surface-300 flex items-center gap-2">
                        <GitBranch size={12} />
                        {repo}
                      </span>
                      <Badge variant="neutral">{count}</Badge>
                    </div>
                  ))}
                {Object.keys(serverStats.events_by_repo).length === 0 && (
                  <p className="text-sm text-surface-500">Sin datos aún</p>
                )}
              </div>
            </div>
          </div>

          <div className="card">
            <h3 className="text-sm font-medium text-white mb-3">Desarrolladores Activos</h3>
            <div className="flex flex-wrap gap-2">
              {Object.entries(serverStats.events_by_developer)
                .sort(([, a], [, b]) => b - a)
                .map(([dev, count]) => (
                  <div key={dev} className="flex items-center gap-2 bg-surface-700 px-3 py-1.5 rounded-lg">
                    <span className="text-sm text-white">{dev}</span>
                    <Badge variant="neutral">{count}</Badge>
                  </div>
                ))}
              {Object.keys(serverStats.events_by_developer).length === 0 && (
                <p className="text-sm text-surface-500">Sin datos aún</p>
              )}
            </div>
          </div>

          <div className="card">
            <h3 className="text-sm font-medium text-white mb-3">Eventos Recientes</h3>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="text-left text-xs text-surface-500 border-b border-surface-700">
                    <th className="pb-2">Hora</th>
                    <th className="pb-2">Desarrollador</th>
                    <th className="pb-2">Acción</th>
                    <th className="pb-2">Rama</th>
                    <th className="pb-2">Estado</th>
                  </tr>
                </thead>
                <tbody>
                  {serverLogs.slice(0, 10).map((log) => (
                    <tr key={log.id} className="border-b border-surface-700/50">
                      <td className="py-2 text-sm text-surface-400">
                        {new Date(log.timestamp).toLocaleString()}
                      </td>
                      <td className="py-2 text-sm text-white">{log.developer_login}</td>
                      <td className="py-2">
                        <Badge variant="neutral">{log.action}</Badge>
                      </td>
                      <td className="py-2 text-sm text-surface-300 font-mono">{log.branch}</td>
                      <td className="py-2">
                        <Badge
                          variant={
                            log.status === 'Success' ? 'success' : log.status === 'Blocked' ? 'danger' : 'warning'
                          }
                        >
                          {log.status === 'Success' ? 'Éxito' : log.status === 'Blocked' ? 'Bloqueado' : 'Fallido'}
                        </Badge>
                      </td>
                    </tr>
                  ))}
                  {serverLogs.length === 0 && (
                    <tr>
                      <td colSpan={5} className="py-4 text-center text-surface-500">
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
