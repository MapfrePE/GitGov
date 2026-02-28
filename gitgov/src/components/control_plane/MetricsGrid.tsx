import { TrendingUp, Activity, AlertTriangle, Users, Info } from 'lucide-react'
import { Bar } from './Bar'

interface MetricsGridProps {
  totalGithubEvents: number
  successRate: string
  activeRepos: number
  desktopPushesToday: number
  githubPushesToday: number
  totalTrackedPushesToday: number
  blockedToday: number
  activeDevsWeek: number
  onOpenActiveDevs?: () => void
}

export function MetricsGrid({
  totalGithubEvents, successRate, activeRepos,
  desktopPushesToday, githubPushesToday, totalTrackedPushesToday,
  blockedToday, activeDevsWeek, onOpenActiveDevs,
}: MetricsGridProps) {
  return (
    <div className="grid grid-cols-4 grid-rows-[auto_auto] gap-3">
      {/* Hero: col-span-2, row-span-2 */}
      <div className="glass-panel col-span-2 row-span-2 p-6 flex flex-col justify-between" style={{ '--stagger': 0 } as React.CSSProperties}>
        <div className="flex items-center gap-2">
          <TrendingUp size={14} strokeWidth={1.5} className="text-brand-400" />
          <span className="card-header">Total Eventos GitHub</span>
        </div>
        <div className="mt-4">
          <span className="text-6xl font-bold text-white tracking-tighter mono-data leading-none">{totalGithubEvents}</span>
        </div>
        <div className="mt-6 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs text-surface-400 uppercase tracking-wide">Tasa éxito</span>
            <span className="text-sm text-success-400 font-semibold mono-data">{successRate}%</span>
          </div>
          <Bar value={parseFloat(successRate)} color="success" />
          <div className="flex items-center gap-4 pt-1">
            <span className="text-xs text-surface-400"><span className="text-surface-200 mono-data">{activeRepos}</span> repos activos</span>
          </div>
        </div>
      </div>

      {/* Pushes */}
      <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 1 } as React.CSSProperties}>
        <div className="flex items-center gap-1.5">
          <Activity size={12} strokeWidth={1.5} className="text-success-400" />
          <span className="card-header">Pushes Hoy</span>
          <span title="métrica diaria UTC" className="inline-flex"><Info size={12} strokeWidth={1.5} className="text-surface-400" /></span>
        </div>
        <div className="mt-4 space-y-2">
          <div className="flex items-end justify-between gap-3">
            <div>
              <div className="text-[11px] text-surface-400 uppercase tracking-wide">Desktop</div>
              <div className="text-2xl font-bold text-white tracking-tighter mono-data leading-none">{desktopPushesToday}</div>
            </div>
            <div className="text-right">
              <div className="text-[11px] text-surface-400 uppercase tracking-wide">GitHub</div>
              <div className="text-2xl font-bold text-surface-300 tracking-tighter mono-data leading-none">{githubPushesToday}</div>
            </div>
          </div>
          <div className="pt-1 border-t border-white/4 flex items-center justify-between text-xs">
            <span className="text-surface-400">Total trazado</span>
            <span className="mono-data text-surface-200 font-medium">{totalTrackedPushesToday}</span>
          </div>
        </div>
      </div>

      {/* Blocked */}
      <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 2 } as React.CSSProperties}>
        <div className="flex items-center gap-1.5">
          <AlertTriangle size={12} strokeWidth={1.5} className="text-danger-400" />
          <span className="card-header">Bloqueados</span>
        </div>
        <span className="text-4xl font-bold text-white tracking-tighter mono-data mt-auto leading-none">{blockedToday}</span>
      </div>

      {/* Devs */}
      <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 3 } as React.CSSProperties}>
        <div className="flex items-center gap-1.5">
          <Users size={12} strokeWidth={1.5} className="text-warning-400" />
          <span className="card-header">Devs Activos 7d</span>
        </div>
        <div className="mt-auto">
          <span className="text-4xl font-bold text-white tracking-tighter mono-data leading-none">{activeDevsWeek}</span>
          {onOpenActiveDevs && (
            <button
              type="button"
              onClick={onOpenActiveDevs}
              className="block mt-2 text-xs text-brand-300 hover:text-brand-200 transition-colors"
            >
              Ver detalle
            </button>
          )}
        </div>
      </div>

      {/* Repos */}
      <div className="glass-panel p-5 flex flex-col justify-between" style={{ '--stagger': 4 } as React.CSSProperties}>
        <span className="card-header">Repos Activos</span>
        <span className="text-4xl font-bold text-white tracking-tighter mono-data mt-auto leading-none">{activeRepos}</span>
        <span className="text-xs text-surface-400 mt-1">últimos 7 días</span>
      </div>
    </div>
  )
}
