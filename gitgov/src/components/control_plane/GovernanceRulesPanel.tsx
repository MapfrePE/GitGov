import { useCallback, useEffect, useMemo, useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Badge } from '@/components/shared/Badge'
import { Button } from '@/components/shared/Button'
import { formatTs } from '@/lib/timezone'
import {
  Shield,
  GitBranch,
  FileText,
  Link2,
  History,
  ChevronDown,
  ChevronRight,
  Zap,
  Building2,
  Lock,
  Save,
  RotateCcw,
  AlertTriangle,
} from 'lucide-react'
import type {
  GitGovConfig,
  EnforcementLevel,
  EnforcementConfig,
  RulesConfig,
  GovernancePreset,
} from '@/lib/types'

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

const PRESET_CONFIGS: Record<Exclude<GovernancePreset, 'custom'>, {
  label: string
  icon: typeof Zap
  description: string
  enforcement: EnforcementConfig
  rules: Partial<RulesConfig>
}> = {
  startup: {
    label: 'Startup',
    icon: Zap,
    description: 'Sin fricción — todo apagado',
    enforcement: { pull_requests: 'off', commits: 'off', branches: 'off', traceability: 'off' },
    rules: {},
  },
  enterprise: {
    label: 'Enterprise',
    icon: Building2,
    description: 'PRs obligatorios, branches protegidos',
    enforcement: { pull_requests: 'warn', commits: 'warn', branches: 'warn', traceability: 'off' },
    rules: { require_pull_request: true, min_approvals: 1, require_conventional_commits: true },
  },
  regulated: {
    label: 'Regulado',
    icon: Lock,
    description: 'Todo bloqueante — compliance total',
    enforcement: { pull_requests: 'block', commits: 'block', branches: 'block', traceability: 'block' },
    rules: {
      require_pull_request: true,
      min_approvals: 2,
      require_conventional_commits: true,
      require_signed_commits: true,
      require_linked_ticket: true,
      block_force_push: true,
    },
  },
}

// ---------------------------------------------------------------------------
// Enforcement toggle
// ---------------------------------------------------------------------------

const ENFORCEMENT_LEVELS: { value: EnforcementLevel; label: string; color: string }[] = [
  { value: 'off', label: 'Off', color: 'bg-surface-700 text-surface-400' },
  { value: 'warn', label: 'Warn', color: 'bg-warning-500/20 text-warning-400 ring-1 ring-warning-500/30' },
  { value: 'block', label: 'Block', color: 'bg-danger-500/20 text-danger-400 ring-1 ring-danger-500/30' },
]

function EnforcementToggle({
  value,
  onChange,
}: {
  value: EnforcementLevel
  onChange: (v: EnforcementLevel) => void
}) {
  return (
    <div className="flex items-center gap-1 rounded-lg bg-surface-900/60 p-0.5 border border-white/5">
      {ENFORCEMENT_LEVELS.map((level) => (
        <button
          key={level.value}
          type="button"
          onClick={() => onChange(level.value)}
          className={`px-2.5 py-1 rounded-md text-[10px] font-semibold uppercase tracking-wider transition-all duration-150 ${
            value === level.value
              ? level.color
              : 'text-surface-500 hover:text-surface-300'
          }`}
        >
          {level.label}
        </button>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Rule toggle row
// ---------------------------------------------------------------------------

function RuleRow({
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  label: string
  description?: string
  checked: boolean
  onChange: (v: boolean) => void
  disabled?: boolean
}) {
  return (
    <label className={`flex items-start gap-3 py-2 px-1 rounded-lg cursor-pointer hover:bg-white/2 transition-colors ${disabled ? 'opacity-40 pointer-events-none' : ''}`}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 accent-brand-500 w-3.5 h-3.5 rounded"
      />
      <div className="flex-1 min-w-0">
        <span className="text-xs text-surface-200 font-medium">{label}</span>
        {description && <p className="text-[10px] text-surface-500 mt-0.5">{description}</p>}
      </div>
    </label>
  )
}

// ---------------------------------------------------------------------------
// Number input row
// ---------------------------------------------------------------------------

function NumberRow({
  label,
  value,
  onChange,
  min,
  max,
  disabled,
}: {
  label: string
  value: number
  onChange: (v: number) => void
  min?: number
  max?: number
  disabled?: boolean
}) {
  return (
    <div className={`flex items-center gap-3 py-2 px-1 ${disabled ? 'opacity-40 pointer-events-none' : ''}`}>
      <span className="text-xs text-surface-200 font-medium flex-1">{label}</span>
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        min={min}
        max={max}
        className="w-16 bg-surface-900 border border-white/10 rounded px-2 py-1 text-xs text-surface-100 text-center"
      />
    </div>
  )
}

// ---------------------------------------------------------------------------
// Collapsible category section
// ---------------------------------------------------------------------------

function CategorySection({
  icon: Icon,
  title,
  enforcement,
  onEnforcementChange,
  defaultOpen,
  children,
}: {
  icon: typeof Shield
  title: string
  enforcement: EnforcementLevel
  onEnforcementChange: (v: EnforcementLevel) => void
  defaultOpen?: boolean
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(defaultOpen ?? true)
  const Chevron = open ? ChevronDown : ChevronRight

  return (
    <div className="rounded-xl border border-white/6 bg-white/[0.02] overflow-hidden">
      <div className="w-full flex items-center gap-3 px-4 py-3 hover:bg-white/[0.02] transition-colors">
        <button
          type="button"
          onClick={() => setOpen(!open)}
          className="flex items-center gap-3 flex-1 min-w-0 text-left"
        >
          <Icon size={14} strokeWidth={1.5} className="text-brand-400 shrink-0" />
          <span className="text-xs font-semibold text-surface-100 flex-1 text-left">{title}</span>
          <Chevron size={12} className="text-surface-500 shrink-0" />
        </button>
        <EnforcementToggle value={enforcement} onChange={onEnforcementChange} />
      </div>
      {open && (
        <div className="px-4 pb-4 pt-1 border-t border-white/5">
          {children}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Tags input for lists (protected branches, patterns, etc.)
// ---------------------------------------------------------------------------

function TagsInput({
  values,
  onChange,
  placeholder,
  disabled,
}: {
  values: string[]
  onChange: (v: string[]) => void
  placeholder?: string
  disabled?: boolean
}) {
  const [input, setInput] = useState('')

  const addTag = () => {
    const val = input.trim()
    if (val && !values.includes(val)) {
      onChange([...values, val])
      setInput('')
    }
  }

  return (
    <div className={`space-y-1.5 ${disabled ? 'opacity-40 pointer-events-none' : ''}`}>
      <div className="flex flex-wrap gap-1">
        {values.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-surface-700/60 text-[10px] text-surface-300 font-mono"
          >
            {tag}
            <button
              type="button"
              onClick={() => onChange(values.filter((t) => t !== tag))}
              className="text-surface-500 hover:text-danger-400 transition-colors"
            >
              ×
            </button>
          </span>
        ))}
      </div>
      <div className="flex gap-1">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), addTag())}
          placeholder={placeholder}
          className="flex-1 bg-surface-900 border border-white/10 rounded px-2 py-1 text-[11px] text-surface-100 font-mono"
        />
        <button
          type="button"
          onClick={addTag}
          className="px-2 py-1 rounded bg-surface-700 text-[10px] text-surface-300 hover:bg-surface-600 transition-colors"
        >
          +
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Default config factory
// ---------------------------------------------------------------------------

function defaultRules(): RulesConfig {
  return {
    require_pull_request: false,
    min_approvals: 0,
    require_conventional_commits: false,
    require_signed_commits: false,
    max_files_per_commit: null,
    require_linked_ticket: false,
    block_force_push: false,
    forbidden_patterns: [],
  }
}

function defaultEnforcement(): EnforcementConfig {
  return { pull_requests: 'off', commits: 'off', branches: 'off', traceability: 'off' }
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function GovernanceRulesPanel({ repoFullName }: { repoFullName: string }) {
  const policyData = useControlPlaneStore((s) => s.policyData)
  const policyHistory = useControlPlaneStore((s) => s.policyHistory)
  const isPolicyLoading = useControlPlaneStore((s) => s.isPolicyLoading)
  const isPolicySaving = useControlPlaneStore((s) => s.isPolicySaving)
  const policyError = useControlPlaneStore((s) => s.policyError)
  const loadPolicy = useControlPlaneStore((s) => s.loadPolicy)
  const savePolicy = useControlPlaneStore((s) => s.savePolicy)
  const loadPolicyHistory = useControlPlaneStore((s) => s.loadPolicyHistory)
  const displayTimezone = useControlPlaneStore((s) => s.displayTimezone)

  // Local draft state
  const [rules, setRules] = useState<RulesConfig>(defaultRules())
  const [enforcement, setEnforcement] = useState<EnforcementConfig>(defaultEnforcement())
  const [protectedBranches, setProtectedBranches] = useState<string[]>([])
  const [branchPatterns, setBranchPatterns] = useState<string[]>([])
  const [forbiddenPatterns, setForbiddenPatterns] = useState<string[]>([])
  const [showHistory, setShowHistory] = useState(false)
  const [saveSuccess, setSaveSuccess] = useState(false)

  // Load policy on mount
  useEffect(() => {
    if (repoFullName) {
      void loadPolicy(repoFullName)
      void loadPolicyHistory(repoFullName)
    }
  }, [repoFullName, loadPolicy, loadPolicyHistory])

  // Sync server data → local draft
  useEffect(() => {
    if (policyData?.config) {
      const cfg = policyData.config
      // Syncing remote policy into local editable draft is intentional here.
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setRules({ ...defaultRules(), ...cfg.rules })
      setEnforcement({ ...defaultEnforcement(), ...cfg.enforcement })
      setProtectedBranches(cfg.branches?.protected ?? [])
      setBranchPatterns(cfg.branches?.patterns ?? [])
      setForbiddenPatterns(cfg.rules?.forbidden_patterns ?? [])
    }
  }, [policyData])

  // Detect active preset
  const activePreset = useMemo<GovernancePreset>(() => {
    for (const [key, preset] of Object.entries(PRESET_CONFIGS)) {
      const e = preset.enforcement
      if (
        enforcement.pull_requests === e.pull_requests &&
        enforcement.commits === e.commits &&
        enforcement.branches === e.branches &&
        enforcement.traceability === e.traceability
      ) {
        return key as GovernancePreset
      }
    }
    return 'custom'
  }, [enforcement])

  const applyPreset = useCallback((preset: Exclude<GovernancePreset, 'custom'>) => {
    const cfg = PRESET_CONFIGS[preset]
    setEnforcement({ ...cfg.enforcement })
    if (cfg.rules) {
      setRules((prev) => ({ ...prev, ...cfg.rules }))
    }
  }, [])

  const handleSave = useCallback(async () => {
    const config: GitGovConfig = {
      branches: {
        patterns: branchPatterns,
        protected: protectedBranches,
      },
      groups: policyData?.config?.groups ?? {},
      admins: policyData?.config?.admins ?? [],
      rules: { ...rules, forbidden_patterns: forbiddenPatterns },
      checklist: policyData?.config?.checklist ?? { confirm: [], auto_check: [] },
      enforcement,
    }
    const ok = await savePolicy(repoFullName, config)
    if (ok) {
      setSaveSuccess(true)
      setTimeout(() => setSaveSuccess(false), 3000)
      void loadPolicyHistory(repoFullName)
    }
  }, [rules, enforcement, protectedBranches, branchPatterns, forbiddenPatterns, policyData, repoFullName, savePolicy, loadPolicyHistory])

  const isDirty = useMemo(() => {
    if (!policyData?.config) return true
    const cfg = policyData.config
    return (
      JSON.stringify(rules) !== JSON.stringify({ ...defaultRules(), ...cfg.rules }) ||
      JSON.stringify(enforcement) !== JSON.stringify({ ...defaultEnforcement(), ...cfg.enforcement }) ||
      JSON.stringify(protectedBranches) !== JSON.stringify(cfg.branches?.protected ?? []) ||
      JSON.stringify(branchPatterns) !== JSON.stringify(cfg.branches?.patterns ?? [])
    )
  }, [rules, enforcement, protectedBranches, branchPatterns, policyData])

  if (isPolicyLoading) {
    return (
      <div className="glass-panel p-5 flex items-center justify-center gap-2 text-surface-500 text-xs">
        <div className="w-3 h-3 border-2 border-surface-600 border-t-brand-400 rounded-full animate-spin" />
        Cargando política de gobierno...
      </div>
    )
  }

  return (
    <div className="glass-panel p-5 space-y-5">
      {/* Header */}
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="card-header">
            <Shield size={13} strokeWidth={1.5} />
            Reglas de Gobierno
          </div>
          <p className="text-[11px] text-surface-400 mt-1">
            Configura las reglas de compliance y niveles de enforcement para{' '}
            <span className="font-mono text-surface-300">{repoFullName}</span>
          </p>
        </div>
        {policyData && (
          <Badge variant="info">
            v{policyData.version} · {policyData.checksum.slice(0, 8)}
          </Badge>
        )}
      </div>

      {/* Error display */}
      {policyError && (
        <div className="flex items-center gap-2 p-2.5 bg-danger-500/10 border border-danger-500/30 rounded-lg text-danger-400 text-xs">
          <AlertTriangle size={12} />
          {policyError}
        </div>
      )}

      {/* Presets */}
      <div>
        <p className="text-[10px] text-surface-500 uppercase tracking-widest font-medium mb-2">Presets</p>
        <div className="flex gap-2">
          {(Object.entries(PRESET_CONFIGS) as [Exclude<GovernancePreset, 'custom'>, typeof PRESET_CONFIGS['startup']][]).map(
            ([key, preset]) => {
              const Icon = preset.icon
              const isActive = activePreset === key
              return (
                <button
                  key={key}
                  type="button"
                  onClick={() => applyPreset(key)}
                  className={`flex-1 flex items-center gap-2 px-3 py-2 rounded-lg border transition-all duration-150 text-left ${
                    isActive
                      ? 'border-brand-500/50 bg-brand-500/10 text-brand-300'
                      : 'border-white/6 bg-white/[0.02] text-surface-400 hover:border-white/12 hover:text-surface-200'
                  }`}
                >
                  <Icon size={13} strokeWidth={1.5} className={isActive ? 'text-brand-400' : ''} />
                  <div>
                    <div className="text-xs font-semibold">{preset.label}</div>
                    <div className="text-[9px] opacity-70">{preset.description}</div>
                  </div>
                </button>
              )
            }
          )}
        </div>
      </div>

      {/* Rule categories */}
      <div className="space-y-2">
        {/* Pull Requests */}
        <CategorySection
          icon={FileText}
          title="Pull Requests"
          enforcement={enforcement.pull_requests}
          onEnforcementChange={(v) => setEnforcement((e) => ({ ...e, pull_requests: v }))}
        >
          <div className="space-y-0.5">
            <RuleRow
              label="Requerir Pull Request"
              description="No se permite push directo a branches protegidos"
              checked={rules.require_pull_request}
              onChange={(v) => setRules((r) => ({ ...r, require_pull_request: v }))}
              disabled={enforcement.pull_requests === 'off'}
            />
            <NumberRow
              label="Aprobaciones mínimas"
              value={rules.min_approvals}
              onChange={(v) => setRules((r) => ({ ...r, min_approvals: v }))}
              min={0}
              max={10}
              disabled={enforcement.pull_requests === 'off'}
            />
          </div>
        </CategorySection>

        {/* Commits */}
        <CategorySection
          icon={FileText}
          title="Commits"
          enforcement={enforcement.commits}
          onEnforcementChange={(v) => setEnforcement((e) => ({ ...e, commits: v }))}
          defaultOpen={false}
        >
          <div className="space-y-0.5">
            <RuleRow
              label="Conventional Commits"
              description="Formato: feat:, fix:, refactor:, etc."
              checked={rules.require_conventional_commits}
              onChange={(v) => setRules((r) => ({ ...r, require_conventional_commits: v }))}
              disabled={enforcement.commits === 'off'}
            />
            <RuleRow
              label="Commits firmados (GPG)"
              description="Requiere firma criptográfica en cada commit"
              checked={rules.require_signed_commits}
              onChange={(v) => setRules((r) => ({ ...r, require_signed_commits: v }))}
              disabled={enforcement.commits === 'off'}
            />
            <NumberRow
              label="Máximo de archivos por commit"
              value={rules.max_files_per_commit ?? 0}
              onChange={(v) => setRules((r) => ({ ...r, max_files_per_commit: v > 0 ? v : null }))}
              min={0}
              max={500}
              disabled={enforcement.commits === 'off'}
            />
            <div className={`py-2 px-1 ${enforcement.commits === 'off' ? 'opacity-40 pointer-events-none' : ''}`}>
              <span className="text-xs text-surface-200 font-medium">Patrones prohibidos</span>
              <p className="text-[10px] text-surface-500 mb-1.5">Bloquea commits con estos patrones en el contenido</p>
              <TagsInput
                values={forbiddenPatterns}
                onChange={setForbiddenPatterns}
                placeholder="ej: password=, secret_key="
                disabled={enforcement.commits === 'off'}
              />
            </div>
          </div>
        </CategorySection>

        {/* Branches */}
        <CategorySection
          icon={GitBranch}
          title="Branches"
          enforcement={enforcement.branches}
          onEnforcementChange={(v) => setEnforcement((e) => ({ ...e, branches: v }))}
          defaultOpen={false}
        >
          <div className="space-y-2">
            <RuleRow
              label="Bloquear force push"
              description="Impide git push --force en branches protegidos"
              checked={rules.block_force_push}
              onChange={(v) => setRules((r) => ({ ...r, block_force_push: v }))}
              disabled={enforcement.branches === 'off'}
            />
            <div className={`py-1 px-1 ${enforcement.branches === 'off' ? 'opacity-40 pointer-events-none' : ''}`}>
              <span className="text-xs text-surface-200 font-medium">Branches protegidos</span>
              <p className="text-[10px] text-surface-500 mb-1.5">Push directo bloqueado en estos branches</p>
              <TagsInput
                values={protectedBranches}
                onChange={setProtectedBranches}
                placeholder="ej: main, staging, production"
                disabled={enforcement.branches === 'off'}
              />
            </div>
            <div className={`py-1 px-1 ${enforcement.branches === 'off' ? 'opacity-40 pointer-events-none' : ''}`}>
              <span className="text-xs text-surface-200 font-medium">Patrones de nombre</span>
              <p className="text-[10px] text-surface-500 mb-1.5">Solo se permiten branches que coincidan con estos patrones</p>
              <TagsInput
                values={branchPatterns}
                onChange={setBranchPatterns}
                placeholder="ej: feat/*, fix/*, hotfix/*"
                disabled={enforcement.branches === 'off'}
              />
            </div>
          </div>
        </CategorySection>

        {/* Traceability */}
        <CategorySection
          icon={Link2}
          title="Trazabilidad"
          enforcement={enforcement.traceability}
          onEnforcementChange={(v) => setEnforcement((e) => ({ ...e, traceability: v }))}
          defaultOpen={false}
        >
          <div className="space-y-0.5">
            <RuleRow
              label="Ticket vinculado obligatorio"
              description="Cada commit/PR debe referenciar un ticket (ej: PROJ-123)"
              checked={rules.require_linked_ticket}
              onChange={(v) => setRules((r) => ({ ...r, require_linked_ticket: v }))}
              disabled={enforcement.traceability === 'off'}
            />
          </div>
        </CategorySection>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-3 pt-1">
        <Button
          variant="primary"
          size="sm"
          loading={isPolicySaving}
          disabled={!isDirty}
          onClick={() => void handleSave()}
        >
          <Save size={13} strokeWidth={1.5} />
          {saveSuccess ? 'Guardado' : 'Guardar política'}
        </Button>
        {isDirty && (
          <button
            type="button"
            onClick={() => {
              if (policyData?.config) {
                const cfg = policyData.config
                setRules({ ...defaultRules(), ...cfg.rules })
                setEnforcement({ ...defaultEnforcement(), ...cfg.enforcement })
                setProtectedBranches(cfg.branches?.protected ?? [])
                setBranchPatterns(cfg.branches?.patterns ?? [])
                setForbiddenPatterns(cfg.rules?.forbidden_patterns ?? [])
              }
            }}
            className="flex items-center gap-1.5 text-[11px] text-surface-400 hover:text-surface-200 transition-colors"
          >
            <RotateCcw size={11} />
            Descartar cambios
          </button>
        )}
        {saveSuccess && (
          <span className="text-[11px] text-success-400 animate-fade-in">Política actualizada correctamente</span>
        )}
      </div>

      {/* History */}
      {policyHistory.length > 0 && (
        <div className="border-t border-white/5 pt-4">
          <button
            type="button"
            onClick={() => setShowHistory(!showHistory)}
            className="flex items-center gap-2 text-[11px] text-surface-400 hover:text-surface-200 transition-colors"
          >
            <History size={11} />
            Historial de cambios ({policyHistory.length})
            {showHistory ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
          </button>
          {showHistory && (
            <div className="mt-3 space-y-1.5">
              {policyHistory.slice(0, 10).map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-center gap-3 px-3 py-2 rounded-lg bg-surface-900/40 border border-white/4 text-[10px]"
                >
                  <Badge variant={entry.change_type === 'create' ? 'success' : 'info'}>
                    {entry.change_type}
                  </Badge>
                  <span className="text-surface-300 flex-1">
                    por <span className="font-mono text-surface-200">{entry.changed_by}</span>
                  </span>
                  <span className="text-surface-500 font-mono">
                    {entry.checksum.slice(0, 8)}
                  </span>
                  <span className="text-surface-500">
                    {formatTs(entry.created_at, displayTimezone)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
