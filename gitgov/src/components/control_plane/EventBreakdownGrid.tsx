import { Badge } from '@/components/shared/Badge'

interface EventBreakdownGridProps {
  githubByType: Record<string, number>
  clientByStatus: Record<string, number>
  commitsWithoutTicket: Array<Record<string, unknown>>
  ticketsWithoutCommits: Array<Record<string, unknown>>
  totalCommitsWithoutTicket: number
  totalTicketsWithoutCommits: number
}

export function EventBreakdownGrid({
  githubByType, clientByStatus,
  commitsWithoutTicket, ticketsWithoutCommits,
  totalCommitsWithoutTicket, totalTicketsWithoutCommits,
}: EventBreakdownGridProps) {
  return (
    <div className="grid grid-cols-4 gap-3">
      {/* GitHub events by type */}
      <div className="glass-panel p-4">
        <div className="card-header mb-3">GitHub por Tipo</div>
        <div className="divide-y divide-white/[0.04]">
          {Object.entries(githubByType).sort(([, a], [, b]) => b - a).slice(0, 5).map(([eventType, count]) => (
            <div key={eventType} className="flex items-center justify-between py-2 first:pt-0 last:pb-0">
              <span className="text-[10px] text-surface-400">{eventType}</span>
              <span className="text-[10px] text-surface-300 mono-data font-medium">{count}</span>
            </div>
          ))}
          {Object.keys(githubByType).length === 0 && <p className="text-[10px] text-surface-600 text-center py-3">Sin datos</p>}
        </div>
      </div>

      {/* Client events by status */}
      <div className="glass-panel p-4">
        <div className="card-header mb-3">Cliente por Estado</div>
        <div className="divide-y divide-white/[0.04]">
          {Object.entries(clientByStatus).sort(([, a], [, b]) => b - a).map(([status, count]) => (
            <div key={status} className="flex items-center justify-between py-2 first:pt-0 last:pb-0">
              <span className="text-[10px] text-surface-400 uppercase tracking-wide">{status}</span>
              <Badge variant={status === 'blocked' ? 'danger' : 'success'}>{count}</Badge>
            </div>
          ))}
          {Object.keys(clientByStatus).length === 0 && <p className="text-[10px] text-surface-600 text-center py-3">Sin datos</p>}
        </div>
      </div>

      {/* Commits without ticket */}
      <div className="glass-panel p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="card-header">Sin ticket</div>
          {totalCommitsWithoutTicket > 0 && <Badge variant="warning">{totalCommitsWithoutTicket}</Badge>}
        </div>
        {commitsWithoutTicket.length > 0 ? (
          <div className="divide-y divide-white/[0.04]">
            {commitsWithoutTicket.map((item, idx) => {
              const sha = typeof item.commit_sha === 'string' ? item.commit_sha.slice(0, 7) : '-'
              const branch = typeof item.branch === 'string' ? item.branch : '-'
              return (
                <div key={`${sha}-${idx}`} className="py-2 first:pt-0">
                  <div className="flex items-center gap-1.5">
                    <code className="text-[9px] text-surface-400 mono-data">{sha}</code>
                    <span className="text-[9px] text-surface-600 mono-data truncate">{branch}</span>
                  </div>
                </div>
              )
            })}
          </div>
        ) : (
          <p className="text-[10px] text-surface-600 text-center py-3">Sin commits faltantes</p>
        )}
      </div>

      {/* Tickets without commits */}
      <div className="glass-panel p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="card-header">Tickets huérfanos</div>
          {totalTicketsWithoutCommits > 0 && <Badge variant="neutral">{totalTicketsWithoutCommits}</Badge>}
        </div>
        {ticketsWithoutCommits.length > 0 ? (
          <div className="divide-y divide-white/[0.04]">
            {ticketsWithoutCommits.map((item, idx) => {
              const ticketId = typeof item.ticket_id === 'string' ? item.ticket_id : '-'
              const status = typeof item.status === 'string' ? item.status : null
              return (
                <div key={`${ticketId}-${idx}`} className="flex items-center gap-1.5 py-2 first:pt-0">
                  <Badge variant="info">{ticketId}</Badge>
                  {status && <Badge variant="warning">{status}</Badge>}
                </div>
              )
            })}
          </div>
        ) : (
          <p className="text-[10px] text-surface-600 text-center py-3">Sin tickets huérfanos</p>
        )}
      </div>
    </div>
  )
}
