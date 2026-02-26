import { Workflow } from 'lucide-react'
import { Bar, formatDurationMs } from './dashboard-helpers'

interface PipelineHealthWidgetProps {
  total: number
  failure: number
  avgDurationMs: number
  reposWithFailures: number
  successRate: string
}

export function PipelineHealthWidget({ total, failure, avgDurationMs, reposWithFailures, successRate }: PipelineHealthWidgetProps) {
  return (
    <div className="glass-panel p-5">
      <div className="card-header mb-4">
        <Workflow size={11} strokeWidth={1.5} className="text-surface-400" />
        Pipeline Health (7d)
      </div>
      {total > 0 ? (
        <div className="space-y-3">
          <div className="flex items-baseline gap-3">
            <span className="text-4xl font-bold text-white tracking-tighter mono-data leading-none">{successRate}%</span>
            <span className="text-[10px] text-surface-500 uppercase tracking-widest">success rate</span>
          </div>
          <Bar value={parseFloat(successRate)} color="success" />
          <div className="grid grid-cols-2 gap-x-8 gap-y-2 pt-2">
            {([
              ['Pipelines', total, ''],
              ['Failures', failure, 'text-danger-400'],
              ['Avg duration', formatDurationMs(avgDurationMs), ''],
              ['Repos w/ failures', reposWithFailures, ''],
            ] as const).map(([label, val, cls]) => (
              <div key={label} className="flex items-center justify-between text-[11px]">
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
  )
}
