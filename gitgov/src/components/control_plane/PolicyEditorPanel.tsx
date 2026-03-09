import { useCallback, useEffect, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Badge } from '@/components/shared/Badge'
import { formatTs } from '@/lib/timezone'
import type { GitGovConfig, EnforcementLevel } from '@/lib/types'
import {
  FileText,
  Shield,
  GitBranch,
  Users,
  CheckSquare,
  AlertTriangle,
  Save,
  History,
  RefreshCw,
  Plus,
  Trash2,
  ChevronDown,
  ChevronRight,
} from 'lucide-react'

const EMPTY_CONFIG: GitGovConfig = {
  branches: { patterns: [], protected: [] },
  groups: {},
  admins: [],
  rules: {
    require_pull_request: false,
    min_approvals: 0,
    require_conventional_commits: false,
    require_signed_commits: false,
    max_files_per_commit: null,
    require_linked_ticket: false,
    block_force_push: false,
    forbidden_patterns: [],
  },
  checklist: { confirm: [], auto_check: [] },
  enforcement: {
    pull_requests: 'off',
    commits: 'off',
    branches: 'off',
    traceability: 'off',
  },
}

function EnforcementSelect({
  label,
  value,
  onChange,
}: {
  label: string
  value: EnforcementLevel
  onChange: (v: EnforcementLevel) => void
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-[11px] text-surface-300">{label}</span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as EnforcementLevel)}
        className="bg-surface-800 border border-white/6 rounded px-2 py-1 text-[11px] text-surface-200 focus:outline-none focus:ring-1 focus:ring-brand-500/50"
      >
        <option value="off">Off</option>
        <option value="warn">Warn</option>
        <option value="block">Block</option>
      </select>
    </div>
  )
}

function TagListEditor({
  label,
  values,
  onChange,
  placeholder,
}: {
  label: string
  values: string[]
  onChange: (v: string[]) => void
  placeholder?: string
}) {
  const [input, setInput] = useState('')

  const add = () => {
    const trimmed = input.trim()
    if (trimmed && !values.includes(trimmed)) {
      onChange([...values, trimmed])
    }
    setInput('')
  }

  return (
    <div className="space-y-1.5">
      <span className="text-[10px] text-surface-500 uppercase tracking-widest font-medium">{label}</span>
      <div className="flex gap-1.5 flex-wrap">
        {values.map((v) => (
          <span
            key={v}
            className="inline-flex items-center gap-1 px-2 py-0.5 bg-surface-800 border border-white/6 rounded text-[11px] text-surface-300"
          >
            {v}
            <button
              type="button"
              onClick={() => onChange(values.filter((x) => x !== v))}
              className="text-surface-600 hover:text-danger-400 transition-colors"
            >
              <Trash2 size={10} />
            </button>
          </span>
        ))}
      </div>
      <div className="flex gap-1.5">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              add()
            }
          }}
          placeholder={placeholder}
          className="flex-1 bg-surface-800 border border-white/6 rounded px-2 py-1 text-[11px] text-surface-200 placeholder:text-surface-600 focus:outline-none focus:ring-1 focus:ring-brand-500/50"
        />
        <button
          type="button"
          onClick={add}
          className="px-2 py-1 bg-surface-800 border border-white/6 rounded text-[11px] text-surface-400 hover:text-surface-200 hover:bg-surface-700 transition-colors"
        >
          <Plus size={12} />
        </button>
      </div>
    </div>
  )
}

function CollapsibleSection({
  icon: Icon,
  title,
  children,
  defaultOpen = false,
}: {
  icon: React.ComponentType<{ size: number; className?: string }>
  title: string
  children: React.ReactNode
  defaultOpen?: boolean
}) {
  const [open, setOpen] = useState(defaultOpen)

  return (
    <div className="border border-white/4 rounded-lg overflow-hidden">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2.5 bg-surface-900/50 hover:bg-surface-800/50 transition-colors"
      >
        {open ? (
          <ChevronDown size={12} className="text-surface-500" />
        ) : (
          <ChevronRight size={12} className="text-surface-500" />
        )}
        <Icon size={14} className="text-brand-400" />
        <span className="text-[12px] font-medium text-surface-200">{title}</span>
      </button>
      {open && <div className="px-3 py-3 space-y-3 bg-surface-950/50">{children}</div>}
    </div>
  )
}

function ToggleRow({
  label,
  checked,
  onChange,
  description,
}: {
  label: string
  checked: boolean
  onChange: (v: boolean) => void
  description?: string
}) {
  return (
    <label className="flex items-start gap-2.5 cursor-pointer group">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 w-3.5 h-3.5 rounded border-white/10 bg-surface-800 text-brand-500 focus:ring-brand-500/30 focus:ring-offset-0"
      />
      <div>
        <span className="text-[11px] text-surface-300 group-hover:text-surface-200 transition-colors">
          {label}
        </span>
        {description && (
          <p className="text-[10px] text-surface-600 mt-0.5">{description}</p>
        )}
      </div>
    </label>
  )
}

export function PolicyEditorPanel() {
  const policyData = useControlPlaneStore((s) => s.policyData)
  const policyHistory = useControlPlaneStore((s) => s.policyHistory)
  const isPolicyLoading = useControlPlaneStore((s) => s.isPolicyLoading)
  const isPolicySaving = useControlPlaneStore((s) => s.isPolicySaving)
  const policyError = useControlPlaneStore((s) => s.policyError)
  const loadPolicy = useControlPlaneStore((s) => s.loadPolicy)
  const savePolicy = useControlPlaneStore((s) => s.savePolicy)
  const loadPolicyHistory = useControlPlaneStore((s) => s.loadPolicyHistory)
  const userRole = useControlPlaneStore((s) => s.userRole)
  const displayTimezone = useControlPlaneStore((s) => s.displayTimezone)
  const isAdmin = userRole === 'Admin'

  const [repoName, setRepoName] = useState('')
  const [config, setConfig] = useState<GitGovConfig>(EMPTY_CONFIG)
  const [dirty, setDirty] = useState(false)
  const [showHistory, setShowHistory] = useState(false)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    if (policyData?.config) {
      setConfig({
        ...EMPTY_CONFIG,
        ...policyData.config,
        branches: { ...EMPTY_CONFIG.branches, ...policyData.config.branches },
        rules: { ...EMPTY_CONFIG.rules, ...policyData.config.rules },
        checklist: { ...EMPTY_CONFIG.checklist, ...policyData.config.checklist },
        enforcement: { ...EMPTY_CONFIG.enforcement, ...policyData.config.enforcement },
      })
      setDirty(false)
      setLoaded(true)
    }
  }, [policyData])

  const handleLoad = useCallback(() => {
    const trimmed = repoName.trim()
    if (!trimmed) return
    void loadPolicy(trimmed)
    void loadPolicyHistory(trimmed)
  }, [repoName, loadPolicy, loadPolicyHistory])

  const handleSave = useCallback(async () => {
    const trimmed = repoName.trim()
    if (!trimmed) return
    const success = await savePolicy(trimmed, config)
    if (success) {
      setDirty(false)
      void loadPolicyHistory(trimmed)
    }
  }, [repoName, config, savePolicy, loadPolicyHistory])

  const updateConfig = useCallback(
    (updater: (prev: GitGovConfig) => GitGovConfig) => {
      setConfig((prev) => {
        const next = updater(prev)
        setDirty(true)
        return next
      })
    },
    [],
  )

  // Group editor helpers
  const [newGroupName, setNewGroupName] = useState('')

  const addGroup = () => {
    const name = newGroupName.trim()
    if (!name || config.groups[name]) return
    updateConfig((c) => ({
      ...c,
      groups: { ...c.groups, [name]: { members: [], allowed_branches: [], allowed_paths: [] } },
    }))
    setNewGroupName('')
  }

  const removeGroup = (name: string) => {
    updateConfig((c) => {
      const { [name]: _, ...rest } = c.groups
      return { ...c, groups: rest }
    })
  }

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <FileText size={16} className="text-brand-400" />
          <h3 className="text-[13px] font-medium text-surface-200">Policy Editor</h3>
          {policyData && (
            <Badge variant="info">v{policyData.version}</Badge>
          )}
          {dirty && <Badge variant="warning">sin guardar</Badge>}
        </div>
        <div className="flex items-center gap-1.5">
          {loaded && (
            <button
              type="button"
              onClick={() => setShowHistory(!showHistory)}
              className="flex items-center gap-1 px-2 py-1 text-[10px] text-surface-400 hover:text-surface-200 bg-surface-800 border border-white/6 rounded transition-colors"
            >
              <History size={11} />
              Historial
            </button>
          )}
        </div>
      </div>

      {/* Repo selector */}
      <div className="flex gap-2">
        <input
          value={repoName}
          onChange={(e) => setRepoName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') handleLoad()
          }}
          placeholder="owner/repo (ej. MapfrePE/GitGov)"
          className="flex-1 bg-surface-900 border border-white/6 rounded-lg px-3 py-2 text-[12px] text-surface-200 placeholder:text-surface-600 focus:outline-none focus:ring-1 focus:ring-brand-500/50"
        />
        <button
          type="button"
          onClick={handleLoad}
          disabled={isPolicyLoading || !repoName.trim()}
          className="px-3 py-2 bg-brand-600 hover:bg-brand-500 disabled:opacity-40 disabled:cursor-not-allowed text-white text-[11px] font-medium rounded-lg transition-colors flex items-center gap-1.5"
        >
          <RefreshCw size={12} className={isPolicyLoading ? 'animate-spin' : ''} />
          Cargar
        </button>
      </div>

      {/* Error */}
      {policyError && (
        <div className="flex items-center gap-2 px-3 py-2 bg-danger-500/10 border border-danger-500/20 rounded-lg">
          <AlertTriangle size={13} className="text-danger-400" />
          <span className="text-[11px] text-danger-300">{policyError}</span>
        </div>
      )}

      {/* Policy not found */}
      {loaded && !policyData && !isPolicyLoading && !policyError && (
        <div className="text-center py-6">
          <FileText size={24} className="text-surface-700 mx-auto mb-2" />
          <p className="text-[11px] text-surface-500">No se encontró política para este repo.</p>
          {isAdmin && (
            <p className="text-[10px] text-surface-600 mt-1">Puedes crear una nueva configurando los campos abajo y guardando.</p>
          )}
        </div>
      )}

      {/* Editor */}
      {(loaded || policyData) && (
        <div className="space-y-2">
          {/* Branches */}
          <CollapsibleSection icon={GitBranch} title="Branches" defaultOpen>
            <TagListEditor
              label="Patrones permitidos"
              values={config.branches.patterns}
              onChange={(v) => updateConfig((c) => ({ ...c, branches: { ...c.branches, patterns: v } }))}
              placeholder="ej. feat/*, fix/*"
            />
            <TagListEditor
              label="Branches protegidos"
              values={config.branches.protected}
              onChange={(v) => updateConfig((c) => ({ ...c, branches: { ...c.branches, protected: v } }))}
              placeholder="ej. main, release/*"
            />
          </CollapsibleSection>

          {/* Rules */}
          <CollapsibleSection icon={Shield} title="Reglas" defaultOpen>
            <ToggleRow
              label="Requerir Pull Request"
              checked={config.rules.require_pull_request}
              onChange={(v) => updateConfig((c) => ({ ...c, rules: { ...c.rules, require_pull_request: v } }))}
            />
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-surface-300">Aprobaciones mínimas</span>
              <input
                type="number"
                min={0}
                max={10}
                value={config.rules.min_approvals}
                onChange={(e) =>
                  updateConfig((c) => ({
                    ...c,
                    rules: { ...c.rules, min_approvals: parseInt(e.target.value) || 0 },
                  }))
                }
                className="w-16 bg-surface-800 border border-white/6 rounded px-2 py-1 text-[11px] text-surface-200 focus:outline-none focus:ring-1 focus:ring-brand-500/50"
              />
            </div>
            <ToggleRow
              label="Conventional Commits"
              checked={config.rules.require_conventional_commits}
              onChange={(v) => updateConfig((c) => ({ ...c, rules: { ...c.rules, require_conventional_commits: v } }))}
              description="Formato: tipo(scope): mensaje"
            />
            <ToggleRow
              label="Commits firmados"
              checked={config.rules.require_signed_commits}
              onChange={(v) => updateConfig((c) => ({ ...c, rules: { ...c.rules, require_signed_commits: v } }))}
            />
            <ToggleRow
              label="Ticket vinculado obligatorio"
              checked={config.rules.require_linked_ticket}
              onChange={(v) => updateConfig((c) => ({ ...c, rules: { ...c.rules, require_linked_ticket: v } }))}
            />
            <ToggleRow
              label="Bloquear force push"
              checked={config.rules.block_force_push}
              onChange={(v) => updateConfig((c) => ({ ...c, rules: { ...c.rules, block_force_push: v } }))}
            />
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-surface-300">Máx archivos por commit</span>
              <input
                type="number"
                min={0}
                max={9999}
                value={config.rules.max_files_per_commit ?? ''}
                onChange={(e) => {
                  const val = e.target.value.trim()
                  updateConfig((c) => ({
                    ...c,
                    rules: { ...c.rules, max_files_per_commit: val ? parseInt(val) || null : null },
                  }))
                }}
                placeholder="sin límite"
                className="w-24 bg-surface-800 border border-white/6 rounded px-2 py-1 text-[11px] text-surface-200 placeholder:text-surface-600 focus:outline-none focus:ring-1 focus:ring-brand-500/50"
              />
            </div>
            <TagListEditor
              label="Patrones prohibidos"
              values={config.rules.forbidden_patterns}
              onChange={(v) => updateConfig((c) => ({ ...c, rules: { ...c.rules, forbidden_patterns: v } }))}
              placeholder="ej. *.env, secrets/*"
            />
          </CollapsibleSection>

          {/* Enforcement */}
          <CollapsibleSection icon={AlertTriangle} title="Enforcement">
            <EnforcementSelect
              label="Pull Requests"
              value={config.enforcement.pull_requests}
              onChange={(v) => updateConfig((c) => ({ ...c, enforcement: { ...c.enforcement, pull_requests: v } }))}
            />
            <EnforcementSelect
              label="Commits"
              value={config.enforcement.commits}
              onChange={(v) => updateConfig((c) => ({ ...c, enforcement: { ...c.enforcement, commits: v } }))}
            />
            <EnforcementSelect
              label="Branches"
              value={config.enforcement.branches}
              onChange={(v) => updateConfig((c) => ({ ...c, enforcement: { ...c.enforcement, branches: v } }))}
            />
            <EnforcementSelect
              label="Trazabilidad"
              value={config.enforcement.traceability}
              onChange={(v) => updateConfig((c) => ({ ...c, enforcement: { ...c.enforcement, traceability: v } }))}
            />
          </CollapsibleSection>

          {/* Groups */}
          <CollapsibleSection icon={Users} title="Grupos">
            {Object.entries(config.groups).map(([name, group]) => (
              <div key={name} className="border border-white/4 rounded-lg p-3 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-[12px] font-medium text-surface-200">{name}</span>
                  {isAdmin && (
                    <button
                      type="button"
                      onClick={() => removeGroup(name)}
                      className="text-surface-600 hover:text-danger-400 transition-colors"
                    >
                      <Trash2 size={12} />
                    </button>
                  )}
                </div>
                <TagListEditor
                  label="Miembros"
                  values={group.members}
                  onChange={(v) =>
                    updateConfig((c) => ({
                      ...c,
                      groups: { ...c.groups, [name]: { ...c.groups[name], members: v } },
                    }))
                  }
                  placeholder="github login"
                />
                <TagListEditor
                  label="Branches permitidos"
                  values={group.allowed_branches}
                  onChange={(v) =>
                    updateConfig((c) => ({
                      ...c,
                      groups: { ...c.groups, [name]: { ...c.groups[name], allowed_branches: v } },
                    }))
                  }
                  placeholder="ej. feat/*, fix/*"
                />
                <TagListEditor
                  label="Paths permitidos"
                  values={group.allowed_paths}
                  onChange={(v) =>
                    updateConfig((c) => ({
                      ...c,
                      groups: { ...c.groups, [name]: { ...c.groups[name], allowed_paths: v } },
                    }))
                  }
                  placeholder="ej. src/*, docs/*"
                />
              </div>
            ))}
            {isAdmin && (
              <div className="flex gap-1.5">
                <input
                  value={newGroupName}
                  onChange={(e) => setNewGroupName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault()
                      addGroup()
                    }
                  }}
                  placeholder="Nombre del grupo"
                  className="flex-1 bg-surface-800 border border-white/6 rounded px-2 py-1 text-[11px] text-surface-200 placeholder:text-surface-600 focus:outline-none focus:ring-1 focus:ring-brand-500/50"
                />
                <button
                  type="button"
                  onClick={addGroup}
                  className="px-2 py-1 bg-surface-800 border border-white/6 rounded text-[11px] text-surface-400 hover:text-surface-200 hover:bg-surface-700 transition-colors flex items-center gap-1"
                >
                  <Plus size={11} /> Agregar
                </button>
              </div>
            )}
          </CollapsibleSection>

          {/* Admins */}
          <CollapsibleSection icon={Shield} title="Admins">
            <TagListEditor
              label="Admin logins"
              values={config.admins}
              onChange={(v) => updateConfig((c) => ({ ...c, admins: v }))}
              placeholder="github login"
            />
          </CollapsibleSection>

          {/* Checklist */}
          <CollapsibleSection icon={CheckSquare} title="Checklist">
            <TagListEditor
              label="Confirmaciones manuales"
              values={config.checklist.confirm}
              onChange={(v) => updateConfig((c) => ({ ...c, checklist: { ...c.checklist, confirm: v } }))}
              placeholder="ej. Tests pasaron localmente"
            />
            <TagListEditor
              label="Auto-check"
              values={config.checklist.auto_check}
              onChange={(v) => updateConfig((c) => ({ ...c, checklist: { ...c.checklist, auto_check: v } }))}
              placeholder="ej. lint, typecheck"
            />
          </CollapsibleSection>

          {/* Save button */}
          {isAdmin && (
            <div className="flex items-center justify-between pt-2">
              <div className="text-[10px] text-surface-600">
                {policyData && (
                  <>
                    Checksum: <span className="mono-data text-surface-500">{policyData.checksum.slice(0, 12)}...</span>
                    {' · '}
                    Actualizado: {formatTs(policyData.updated_at, displayTimezone)}
                  </>
                )}
              </div>
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={isPolicySaving || !dirty || !repoName.trim()}
                className="px-4 py-2 bg-brand-600 hover:bg-brand-500 disabled:opacity-40 disabled:cursor-not-allowed text-white text-[11px] font-medium rounded-lg transition-colors flex items-center gap-1.5"
              >
                <Save size={12} />
                {isPolicySaving ? 'Guardando...' : 'Guardar política'}
              </button>
            </div>
          )}

          {!isAdmin && (
            <div className="flex items-center gap-2 px-3 py-2 bg-surface-900/50 border border-white/4 rounded-lg">
              <Shield size={12} className="text-surface-500" />
              <span className="text-[10px] text-surface-500">Solo los administradores pueden editar políticas.</span>
            </div>
          )}
        </div>
      )}

      {/* History panel */}
      {showHistory && policyHistory.length > 0 && (
        <div className="border border-white/4 rounded-lg overflow-hidden">
          <div className="px-3 py-2 bg-surface-900/50 flex items-center gap-2">
            <History size={13} className="text-surface-500" />
            <span className="text-[11px] font-medium text-surface-300">Historial de cambios</span>
          </div>
          <div className="max-h-[200px] overflow-auto">
            <table className="w-full">
              <thead className="sticky top-0 bg-surface-800">
                <tr className="text-left text-[9px] text-surface-600 uppercase tracking-widest">
                  <th className="py-1.5 px-3 font-medium">Fecha</th>
                  <th className="py-1.5 px-3 font-medium">Tipo</th>
                  <th className="py-1.5 px-3 font-medium">Por</th>
                  <th className="py-1.5 px-3 font-medium">Checksum</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/3">
                {policyHistory.map((entry) => (
                  <tr key={entry.id} className="hover:bg-white/2">
                    <td className="py-1.5 px-3 text-[10px] text-surface-400">
                      {formatTs(entry.created_at, displayTimezone)}
                    </td>
                    <td className="py-1.5 px-3">
                      <Badge variant={entry.change_type === 'override' ? 'warning' : 'neutral'}>
                        {entry.change_type}
                      </Badge>
                    </td>
                    <td className="py-1.5 px-3 text-[10px] text-surface-300">{entry.changed_by}</td>
                    <td className="py-1.5 px-3 text-[10px] text-surface-500 mono-data">
                      {entry.checksum.slice(0, 10)}...
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {showHistory && policyHistory.length === 0 && (
        <div className="text-center py-4 text-[11px] text-surface-600">Sin historial de cambios.</div>
      )}
    </div>
  )
}
