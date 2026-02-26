import { Button } from '@/components/shared/Button'

interface DashboardHeaderProps {
  autoRefresh: boolean
  onAutoRefreshChange: (value: boolean) => void
  onRefresh: () => void
  isRefreshing: boolean
}

export function DashboardHeader({ autoRefresh, onAutoRefreshChange, onRefresh, isRefreshing }: DashboardHeaderProps) {
  return (
    <div className="flex items-center justify-between">
      <div>
        <h2 className="text-sm font-semibold text-white tracking-tight">Dashboard</h2>
        <p className="text-[10px] text-surface-600">Control Plane overview</p>
      </div>
      <div className="flex items-center gap-3">
        <label className="flex items-center gap-1.5 text-[10px] text-surface-500 cursor-pointer select-none">
          <input type="checkbox" checked={autoRefresh} onChange={(e) => onAutoRefreshChange(e.target.checked)} className="rounded border-surface-700 bg-transparent text-brand-500 focus:ring-brand-500/20 w-3 h-3" />
          Auto-refresh
        </label>
        <Button variant="ghost" size="sm" onClick={onRefresh} loading={isRefreshing}>
          Actualizar
        </Button>
      </div>
    </div>
  )
}
