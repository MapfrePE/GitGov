import { useState, useEffect } from 'react'
import { Ticket } from 'lucide-react'
import { Button } from '@/components/shared/Button'
import { Bar } from './dashboard-helpers'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'

export function TicketCoverageWidget() {
  const {
    ticketCoverage, jiraCoverageFilters,
    applyTicketCoverageFilters, correlateJiraTickets, loadLogs,
  } = useControlPlaneStore()

  const [isCorrelatingJira, setIsCorrelatingJira] = useState(false)
  const [ticketHours, setTicketHours] = useState(jiraCoverageFilters.hours)
  const [ticketRepoFilter, setTicketRepoFilter] = useState(jiraCoverageFilters.repo_full_name)
  const [ticketBranchFilter, setTicketBranchFilter] = useState(jiraCoverageFilters.branch)

  useEffect(() => {
    setTicketHours(jiraCoverageFilters.hours)
    setTicketRepoFilter(jiraCoverageFilters.repo_full_name)
    setTicketBranchFilter(jiraCoverageFilters.branch)
  }, [jiraCoverageFilters])

  const handleCorrelate = async () => {
    setIsCorrelatingJira(true)
    try {
      await correlateJiraTickets({ hours: jiraCoverageFilters.hours, limit: 500, repo_full_name: jiraCoverageFilters.repo_full_name.trim() || undefined })
      await loadLogs(50)
    } finally {
      setIsCorrelatingJira(false)
    }
  }

  return (
    <div className="glass-panel p-5 flex flex-col">
      <div className="flex items-center justify-between mb-3">
        <div className="card-header">
          <Ticket size={11} strokeWidth={1.5} className="text-surface-400" />
          Ticket Coverage
        </div>
        <Button variant="ghost" size="sm" loading={isCorrelatingJira} onClick={handleCorrelate}>
          Correlacionar
        </Button>
      </div>
      {/* Filters row */}
      <div className="flex gap-1.5 mb-2">
        <input value={ticketRepoFilter} onChange={(e) => setTicketRepoFilter(e.target.value)} placeholder="repo" className="flex-1 min-w-0 rounded-lg bg-white/3 border border-white/6 px-2 py-1.5 text-xs text-white placeholder:text-surface-500 focus:border-brand-500/40 focus:outline-none transition-colors" />
        <input value={ticketBranchFilter} onChange={(e) => setTicketBranchFilter(e.target.value)} placeholder="rama" className="flex-1 min-w-0 rounded-lg bg-white/3 border border-white/6 px-2 py-1.5 text-xs text-white placeholder:text-surface-500 focus:border-brand-500/40 focus:outline-none transition-colors" />
        <select value={ticketHours} onChange={(e) => setTicketHours(Number(e.target.value))} className="w-16 shrink-0 rounded-lg bg-white/3 border border-white/6 px-1.5 py-1.5 text-xs text-white focus:border-brand-500/40 focus:outline-none transition-colors">
          <option value={24}>24h</option>
          <option value={72}>72h</option>
          <option value={168}>7d</option>
          <option value={720}>30d</option>
        </select>
      </div>
      <div className="flex items-center justify-end gap-1.5 mb-3">
        <Button variant="ghost" size="sm" onClick={() => { setTicketHours(72); setTicketRepoFilter(''); setTicketBranchFilter('') }}>Limpiar</Button>
        <Button variant="secondary" size="sm" onClick={() => applyTicketCoverageFilters({ hours: ticketHours, repo_full_name: ticketRepoFilter.trim(), branch: ticketBranchFilter.trim() })}>Aplicar</Button>
      </div>
      {/* Coverage data */}
      {ticketCoverage && ticketCoverage.total_commits > 0 ? (
        <div className="space-y-3 mt-auto">
          <div className="flex items-baseline gap-2">
            <span className="text-3xl font-bold text-white tracking-tighter mono-data leading-none">{ticketCoverage.coverage_percentage.toFixed(1)}%</span>
            <span className="text-xs text-surface-400">cobertura</span>
          </div>
          <Bar value={ticketCoverage.coverage_percentage} />
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <div className="flex justify-between"><span className="text-surface-400">Commits</span><span className="text-surface-200 mono-data">{ticketCoverage.total_commits}</span></div>
            <div className="flex justify-between"><span className="text-surface-400">Con ticket</span><span className="text-success-400 mono-data">{ticketCoverage.commits_with_ticket}</span></div>
            <div className="flex justify-between"><span className="text-surface-400">Sin ticket</span><span className="text-warning-400 mono-data">{ticketCoverage.commits_without_ticket.length}</span></div>
            <div className="flex justify-between"><span className="text-surface-400">Huérfanos</span><span className="text-surface-200 mono-data">{ticketCoverage.tickets_without_commits.length}</span></div>
          </div>
        </div>
      ) : (
        <div className="py-8 text-center mt-auto">
          <Ticket size={18} strokeWidth={1.5} className="mx-auto text-surface-700 mb-2" />
          <p className="text-xs text-surface-400">Sin datos de cobertura</p>
        </div>
      )}
    </div>
  )
}
