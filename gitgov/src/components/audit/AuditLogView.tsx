import { useState, useEffect } from 'react'
import type { AuditFilter } from '@/lib/types'
import { useAuditStore } from '@/store/useAuditStore'
import { useAuthStore } from '@/store/useAuthStore'
import { AuditLogRow } from './AuditLogRow'
import { Button } from '@/components/shared/Button'
import { Spinner } from '@/components/shared/Spinner'
import { RefreshCw, Users, AlertTriangle, TrendingUp } from 'lucide-react'

export function AuditLogView() {
  const { user } = useAuthStore()
  const { logs, stats, isLoading, loadLogs, loadStats, setFilter } = useAuditStore()
  const [dateFrom, setDateFrom] = useState('')
  const [dateTo, setDateTo] = useState('')

  useEffect(() => {
    if (user?.is_admin) {
      loadLogs(true)
      loadStats()
    }
  }, [user?.is_admin, loadLogs, loadStats])

  const handleRefresh = () => {
    loadLogs(true)
    loadStats()
  }

  const handleDateFilter = () => {
    const newFilter: Partial<AuditFilter> = {}
    if (dateFrom) {
      newFilter.start_date = new Date(dateFrom).getTime()
    }
    if (dateTo) {
      newFilter.end_date = new Date(dateTo).setHours(23, 59, 59, 999)
    }
    setFilter(newFilter)
    loadLogs(true)
  }

  if (!user?.is_admin) {
    return (
      <div className="p-8 text-center text-surface-400">
        No tienes permisos para ver esta página
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col">
      {/* Stats */}
      {stats && (
        <div className="grid grid-cols-4 gap-4 p-4 border-b border-surface-700">
          <StatCard
            icon={<TrendingUp size={20} />}
            label="Pushes hoy"
            value={stats.pushes_today}
            color="brand"
          />
          <StatCard
            icon={<AlertTriangle size={20} />}
            label="Bloqueados hoy"
            value={stats.blocked_today}
            color="danger"
          />
          <StatCard
            icon={<Users size={20} />}
            label="Devs activos (semana)"
            value={stats.active_devs_this_week}
            color="success"
          />
          <StatCard
            icon={<RefreshCw size={20} />}
            label="Acción frecuente"
            value={stats.most_frequent_action ?? '-'}
            color="warning"
          />
        </div>
      )}

      {/* Filters */}
      <div className="flex items-center gap-4 p-4 border-b border-surface-700">
        <div className="flex items-center gap-2">
          <label className="text-sm text-surface-400">Desde:</label>
          <input
            type="date"
            value={dateFrom}
            onChange={(e) => setDateFrom(e.target.value)}
            className="px-2 py-1 bg-surface-800 border border-surface-700 rounded text-sm text-white"
          />
        </div>
        <div className="flex items-center gap-2">
          <label className="text-sm text-surface-400">Hasta:</label>
          <input
            type="date"
            value={dateTo}
            onChange={(e) => setDateTo(e.target.value)}
            className="px-2 py-1 bg-surface-800 border border-surface-700 rounded text-sm text-white"
          />
        </div>
        <Button size="sm" onClick={handleDateFilter}>
          Filtrar
        </Button>
        <Button size="sm" variant="ghost" onClick={handleRefresh}>
          <RefreshCw size={14} className="mr-1" />
          Actualizar
        </Button>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-full">
            <Spinner size="lg" />
          </div>
        ) : logs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-surface-500">
            No hay logs para mostrar
          </div>
        ) : (
          <table className="w-full">
            <thead className="bg-surface-800 sticky top-0">
              <tr className="text-left text-sm text-surface-400">
                <th className="px-4 py-2 font-medium">Fecha</th>
                <th className="px-4 py-2 font-medium">Usuario</th>
                <th className="px-4 py-2 font-medium">Acción</th>
                <th className="px-4 py-2 font-medium">Rama</th>
                <th className="px-4 py-2 font-medium">Archivos</th>
                <th className="px-4 py-2 font-medium">Estado</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((log) => (
                <AuditLogRow key={log.id} log={log} />
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}

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
    <div className="bg-surface-800 rounded-lg p-4">
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-2 rounded ${colorClasses[color]}`}>{icon}</div>
        <span className="text-sm text-surface-400">{label}</span>
      </div>
      <p className="text-2xl font-bold text-white">{value}</p>
    </div>
  )
}
