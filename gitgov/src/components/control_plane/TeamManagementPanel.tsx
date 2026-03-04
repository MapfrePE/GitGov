import { useEffect, useMemo, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Badge } from '@/components/shared/Badge'
import { formatTs } from '@/lib/timezone'

export function TeamManagementPanel() {
  const selectedOrgName = useControlPlaneStore((s) => s.selectedOrgName)
  const teamOverview = useControlPlaneStore((s) => s.teamOverview)
  const teamOverviewTotal = useControlPlaneStore((s) => s.teamOverviewTotal)
  const teamRepos = useControlPlaneStore((s) => s.teamRepos)
  const teamReposTotal = useControlPlaneStore((s) => s.teamReposTotal)
  const teamWindowDays = useControlPlaneStore((s) => s.teamWindowDays)
  const teamStatusFilter = useControlPlaneStore((s) => s.teamStatusFilter)
  const setTeamFilters = useControlPlaneStore((s) => s.setTeamFilters)
  const loadTeamOverview = useControlPlaneStore((s) => s.loadTeamOverview)
  const loadTeamRepos = useControlPlaneStore((s) => s.loadTeamRepos)
  const displayTimezone = useControlPlaneStore((s) => s.displayTimezone)

  const [daysInput, setDaysInput] = useState(String(teamWindowDays))
  const [activeTab, setActiveTab] = useState<'devs' | 'repos'>('devs')

  useEffect(() => {
    const orgName = selectedOrgName.trim() || undefined
    void Promise.all([
      loadTeamOverview({ orgName }),
      loadTeamRepos({ orgName }),
    ])
  }, [selectedOrgName, loadTeamOverview, loadTeamRepos])

  const totals = useMemo(() => {
    return teamOverview.reduce(
      (acc, dev) => {
        acc.events += dev.total_events
        acc.commits += dev.commits
        acc.pushes += dev.pushes
        acc.blocked += dev.blocked_pushes
        return acc
      },
      { events: 0, commits: 0, pushes: 0, blocked: 0 },
    )
  }, [teamOverview])

  return (
    <div className="glass-panel p-5 space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <div className="card-header">Gestión de Equipo</div>
          <p className="text-xs text-surface-400 mt-1">Admin: visibilidad de developers y repos por actividad real.</p>
        </div>
        <div className="flex items-center gap-2 text-[11px]">
          <Badge variant="neutral">Devs: {teamOverviewTotal}</Badge>
          <Badge variant="neutral">Repos: {teamReposTotal}</Badge>
          <Badge variant="neutral">Eventos: {totals.events}</Badge>
        </div>
      </div>

      <div className="rounded-lg border border-white/8 p-3 bg-white/2 grid grid-cols-1 md:grid-cols-4 gap-2">
        <input
          value={selectedOrgName}
          readOnly
          className="bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-300"
          title="Org activa"
        />
        <input
          value={daysInput}
          onChange={(e) => setDaysInput(e.target.value)}
          className="bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          placeholder="days"
        />
        <select
          value={teamStatusFilter}
          onChange={(e) => setTeamFilters({ status: e.target.value as '' | 'active' | 'disabled' })}
          className="bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
        >
          <option value="">Todos</option>
          <option value="active">Active</option>
          <option value="disabled">Disabled</option>
        </select>
        <button
          type="button"
          className="px-3 py-1.5 rounded bg-brand-500/20 border border-brand-500/40 text-brand-200 text-xs"
          onClick={async () => {
            const days = Number.parseInt(daysInput, 10)
            if (Number.isFinite(days)) {
              setTeamFilters({ days })
            }
            await Promise.all([
              loadTeamOverview({ orgName: selectedOrgName, days: Number.isFinite(days) ? days : undefined }),
              loadTeamRepos({ orgName: selectedOrgName, days: Number.isFinite(days) ? days : undefined }),
            ])
          }}
        >
          Aplicar filtros
        </button>
      </div>

      <div className="flex gap-2">
        <button
          type="button"
          className={`px-3 py-1.5 rounded text-xs border ${activeTab === 'devs' ? 'bg-brand-500/20 border-brand-500/40 text-brand-200' : 'bg-white/5 border-white/10 text-surface-300'}`}
          onClick={() => setActiveTab('devs')}
        >
          Developers
        </button>
        <button
          type="button"
          className={`px-3 py-1.5 rounded text-xs border ${activeTab === 'repos' ? 'bg-brand-500/20 border-brand-500/40 text-brand-200' : 'bg-white/5 border-white/10 text-surface-300'}`}
          onClick={() => setActiveTab('repos')}
        >
          Repos
        </button>
      </div>

      {activeTab === 'devs' && (
        <div className="max-h-80 overflow-auto border border-white/6 rounded-lg">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-surface-800">
              <tr className="text-surface-500 text-[10px]">
                <th className="text-left py-2 px-2">Developer</th>
                <th className="text-left py-2 px-2">Rol/Estado</th>
                <th className="text-left py-2 px-2">Actividad</th>
                <th className="text-left py-2 px-2">Repos activos</th>
                <th className="text-left py-2 px-2">Último evento</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {teamOverview.map((dev) => (
                <tr key={dev.login}>
                  <td className="py-1.5 px-2 text-surface-100">
                    <div>{dev.login}</div>
                    <div className="text-[10px] text-surface-500">{dev.email || '-'}</div>
                  </td>
                  <td className="py-1.5 px-2 text-surface-300">
                    <div>{dev.role}</div>
                    <div className="text-[10px]">{dev.status}</div>
                  </td>
                  <td className="py-1.5 px-2 text-surface-300">
                    <div>events: {dev.total_events}</div>
                    <div className="text-[10px]">commits: {dev.commits} | pushes: {dev.pushes} | blocked: {dev.blocked_pushes}</div>
                  </td>
                  <td className="py-1.5 px-2">
                    <div className="text-surface-200">{dev.repos_active_count}</div>
                    <div className="text-[10px] text-surface-500 truncate max-w-64" title={dev.repos.slice(0, 3).map((r) => r.repo_name).join(', ')}>
                      {dev.repos.slice(0, 3).map((r) => r.repo_name).join(', ') || '-'}
                    </div>
                  </td>
                  <td className="py-1.5 px-2 text-surface-400">{formatTs(dev.last_seen, displayTimezone)}</td>
                </tr>
              ))}
              {teamOverview.length === 0 && (
                <tr>
                  <td colSpan={5} className="py-8 text-center text-surface-600">Sin actividad para los filtros actuales.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {activeTab === 'repos' && (
        <div className="max-h-80 overflow-auto border border-white/6 rounded-lg">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-surface-800">
              <tr className="text-surface-500 text-[10px]">
                <th className="text-left py-2 px-2">Repo</th>
                <th className="text-left py-2 px-2">Developers activos</th>
                <th className="text-left py-2 px-2">Eventos</th>
                <th className="text-left py-2 px-2">Commits/Pushes/Blocked</th>
                <th className="text-left py-2 px-2">Último evento</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {teamRepos.map((repo) => (
                <tr key={repo.repo_name}>
                  <td className="py-1.5 px-2 text-surface-100">{repo.repo_name}</td>
                  <td className="py-1.5 px-2 text-surface-300">{repo.developers_active}</td>
                  <td className="py-1.5 px-2 text-surface-300">{repo.total_events}</td>
                  <td className="py-1.5 px-2 text-surface-300">{repo.commits}/{repo.pushes}/{repo.blocked_pushes}</td>
                  <td className="py-1.5 px-2 text-surface-400">{formatTs(repo.last_seen, displayTimezone)}</td>
                </tr>
              ))}
              {teamRepos.length === 0 && (
                <tr>
                  <td colSpan={5} className="py-8 text-center text-surface-600">Sin repos para los filtros actuales.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
