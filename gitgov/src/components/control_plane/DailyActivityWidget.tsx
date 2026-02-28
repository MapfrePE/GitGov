import { CalendarDays } from 'lucide-react'

interface DailyActivityPoint {
  day: string
  commits: number
  pushes: number
}

interface DailyActivityWidgetProps {
  points: DailyActivityPoint[]
}

export function DailyActivityWidget({ points }: DailyActivityWidgetProps) {
  const ordered = [...points].reverse()
  const maxValue = Math.max(1, ...ordered.map((p) => Math.max(p.commits, p.pushes)))
  const totalCommits = ordered.reduce((acc, p) => acc + p.commits, 0)
  const totalPushes = ordered.reduce((acc, p) => acc + p.pushes, 0)

  return (
    <div className="glass-panel p-5">
      <div className="card-header mb-4">
        <CalendarDays size={11} strokeWidth={1.5} className="text-surface-400" />
        Actividad diaria (UTC)
      </div>

      {ordered.length === 0 ? (
        <div className="py-10 text-center">
          <CalendarDays size={20} strokeWidth={1.5} className="mx-auto text-surface-700 mb-2" />
          <p className="text-[11px] text-surface-600">Sin actividad registrada</p>
        </div>
      ) : (
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3 text-[10px]">
            <div className="rounded border border-white/5 bg-black/10 px-2 py-1.5">
              <div className="text-surface-600 uppercase tracking-widest">Commits</div>
              <div className="text-surface-300 mono-data font-semibold">{totalCommits}</div>
            </div>
            <div className="rounded border border-white/5 bg-black/10 px-2 py-1.5">
              <div className="text-surface-600 uppercase tracking-widest">Pushes</div>
              <div className="text-surface-300 mono-data font-semibold">{totalPushes}</div>
            </div>
          </div>

          <div className="flex items-end gap-1 h-24">
            {ordered.map((point) => {
              const commitHeight = Math.max(4, (point.commits / maxValue) * 100)
              const pushHeight = Math.max(4, (point.pushes / maxValue) * 100)
              return (
                <div key={point.day} className="flex-1 min-w-0">
                  <div className="h-20 flex items-end justify-center gap-0.5">
                    <div className="w-1.5 rounded-t bg-brand-500/70" style={{ height: `${commitHeight}%` }} />
                    <div className="w-1.5 rounded-t bg-success-500/70" style={{ height: `${pushHeight}%` }} />
                  </div>
                  <div className="mt-1 text-center text-[9px] text-surface-600 mono-data truncate">
                    {point.day.slice(5)}
                  </div>
                </div>
              )
            })}
          </div>

          <div className="flex items-center gap-3 text-[10px] text-surface-500">
            <span className="inline-flex items-center gap-1">
              <span className="h-2 w-2 rounded bg-brand-500/70" />
              commits
            </span>
            <span className="inline-flex items-center gap-1">
              <span className="h-2 w-2 rounded bg-success-500/70" />
              pushes
            </span>
            <span className="ml-auto mono-data">N={ordered.length} días</span>
          </div>
        </div>
      )}
    </div>
  )
}
