import { useState, useMemo, useEffect } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import { useAuthStore } from '@/store/useAuthStore'
import { Button } from '@/components/shared/Button'
import { COMMIT_TYPES } from '@/lib/constants'
import { AlertTriangle, ArrowDown, ArrowUp, GitCommit, Upload, RotateCcw } from 'lucide-react'
import { toast } from '@/components/shared/Toast'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import clsx from 'clsx'

interface GitIdentity {
  name: string | null
  email: string | null
}

function formatPushErrorForUser(rawError: unknown): string {
  const parsed = parseCommandError(String(rawError))
  const msg = parsed.message || String(rawError)

  if (msg.includes('without `workflow` scope') || msg.includes('without workflow scope')) {
    return 'Push rechazado por GitHub: estás modificando .github/workflows/* y tu token no tiene permiso "workflow". Reautentícate en GitHub para conceder ese permiso y vuelve a intentar.'
  }

  if (msg.includes('Invalid username or token') || msg.includes('Authentication failed')) {
    return 'Push rechazado por GitHub: token inválido o expirado. Reautentícate en GitHub y vuelve a intentar.'
  }

  return msg
}

export function CommitPanel() {
  const {
    repoPath,
    stagedFiles,
    fileChanges,
    currentBranch,
    branchSync,
    commit,
    push,
    unstageAll,
    refreshStatus,
    refreshBranchSync,
  } = useRepoStore()
  const { user } = useAuthStore()
  const [message, setMessage] = useState('')
  const [commitType, setCommitType] = useState('feat')
  const [isCommitting, setIsCommitting] = useState(false)
  const [isPushing, setIsPushing] = useState(false)
  const [lastCommitHash, setLastCommitHash] = useState<string | null>(null)
  const [gitIdentity, setGitIdentity] = useState<GitIdentity | null>(null)

  useEffect(() => {
    if (!repoPath) { setGitIdentity(null); return }
    tauriInvoke<GitIdentity>('cmd_get_git_identity', { repoPath })
      .then(setGitIdentity)
      .catch(() => setGitIdentity(null))
  }, [repoPath])

  // Mismatch: la identidad local del repo difiere de la cuenta GitHub autenticada.
  // La Desktop App siempre usa el usuario autenticado para commits, pero si el dev
  // tambien usa git CLI en este repo, sus commits tendran el autor del git config local.
  const identityMismatch = (() => {
    if (!gitIdentity || !user) return null
    const localEmail = gitIdentity.email ?? ''
    const localName  = gitIdentity.name  ?? ''
    const emailOk  = localEmail.toLowerCase().includes(user.login.toLowerCase())
    const nameOk   = localName.toLowerCase().includes(user.login.toLowerCase()) ||
                     (user.name && localName.toLowerCase().includes(user.name.toLowerCase()))
    if (!localName || !localEmail) {
      return { localName, localEmail, reason: 'incomplete' as const }
    }
    if (!emailOk && !nameOk) {
      return { localName, localEmail, reason: 'mismatch' as const }
    }
    return null
  })()

  const fullMessage = useMemo(() => {
    if (!message.trim()) return ''
    if (message.includes(':')) return message
    return `${commitType}: ${message}`
  }, [commitType, message])

  const isValidMessage = useMemo(() => {
    if (!fullMessage) return false
    return /^(feat|fix|docs|style|refactor|test|chore|hotfix):/.test(fullMessage)
  }, [fullMessage])

  const ahead = branchSync?.ahead ?? 0
  const behind = branchSync?.behind ?? 0
  const hasUpstream = branchSync?.has_upstream ?? false
  const hasLocalCommits = ahead > 0

  const hasStagedFiles = stagedFiles.size > 0
  const hasUncommittedChanges = fileChanges.some((f) => f.staged) || stagedFiles.size > 0
  const canPush = Boolean(currentBranch) && (hasLocalCommits || lastCommitHash !== null || hasUncommittedChanges)
  const isIdentityBlocked = Boolean(identityMismatch)

  const handleCommit = async () => {
    if (!user || !isValidMessage) return
    if (isIdentityBlocked) {
      toast('error', 'Commit bloqueado: la identidad git local no coincide con tu cuenta GitGov. Corrige user.name y user.email del repo para continuar.')
      return
    }
    setIsCommitting(true)
    try {
      const hash = await commit(
        fullMessage,
        user.name || user.login,
        `${user.login}@users.noreply.github.com`,
        user.login
      )
      setLastCommitHash(hash)
      setMessage('')
      toast('success', `Commit creado: ${hash.substring(0, 7)}`)
      const sync = await refreshBranchSync(currentBranch ?? undefined)
      const aheadAfterCommit = sync?.ahead ?? 0
      if (aheadAfterCommit > 0) {
        toast(
          'warning',
          `Tienes ${aheadAfterCommit} commit(s) local(es) sin push en ${sync?.branch ?? currentBranch ?? 'la rama actual'}.`
        )
      }
    } catch (e) {
      toast('error', parseCommandError(String(e)).message)
    } finally {
      setIsCommitting(false)
    }
  }

  const handlePush = async () => {
    if (!user || !currentBranch) return
    if (isIdentityBlocked) {
      toast('error', 'Push bloqueado: la identidad git local no coincide con tu cuenta GitGov. Corrige user.name y user.email del repo para continuar.')
      return
    }
    setIsPushing(true)
    try {
      await push(currentBranch, user.login)
      const syncAfterPush = await refreshBranchSync(currentBranch)
      const aheadAfterPush = syncAfterPush?.ahead ?? 0
      if (aheadAfterPush > 0) {
        toast(
          'warning',
          `Push ejecutado pero aún quedan ${aheadAfterPush} commit(s) sin sincronizar en ${syncAfterPush?.branch ?? currentBranch}.`
        )
      } else {
        toast('success', `Push exitoso a ${currentBranch}`)
      }
      setLastCommitHash(null)
      await refreshStatus()
    } catch (e) {
      toast('error', formatPushErrorForUser(e))
      const syncAfterError = await refreshBranchSync(currentBranch)
      const aheadAfterError = syncAfterError?.ahead ?? 0
      if (aheadAfterError > 0) {
        toast(
          'warning',
          `Alerta: tienes ${aheadAfterError} commit(s) local(es) sin push en ${syncAfterError?.branch ?? currentBranch}.`
        )
      }
    } finally {
      setIsPushing(false)
    }
  }

  const handleUnstageAll = async () => {
    await unstageAll()
    toast('info', 'Staging area limpiado')
  }

  return (
    <div className="border-t border-surface-700/30 bg-surface-900/50 px-5 py-4">
      {identityMismatch && (
        <div className="mb-3 flex items-start gap-2 rounded-lg border border-warning-500/30 bg-warning-500/10 px-3 py-2">
          <AlertTriangle size={13} strokeWidth={1.75} className="mt-0.5 shrink-0 text-warning-400" />
          <div className="min-w-0">
            <p className="text-[11px] font-medium text-warning-300">
              {identityMismatch.reason === 'incomplete'
                ? 'Identidad git incompleta en este repo'
                : 'Identidad git difiere de tu cuenta GitGov'}
            </p>
            <p className="mt-0.5 text-[10px] text-surface-400">
              {identityMismatch.reason === 'incomplete'
                ? 'Commits por CLI no tendrán autor válido. '
                : `CLI usará "${identityMismatch.localName} <${identityMismatch.localEmail}>" en vez de @${user?.login}. `}
              Ejecuta{' '}
              <code className="rounded bg-surface-800 px-1 text-warning-300">
                git config --local user.name "Tu Nombre"
              </code>{' '}
              y{' '}
              <code className="rounded bg-surface-800 px-1 text-warning-300">
                git config --local user.email "tu@email.com"
              </code>
              {' '}o{' '}
              <code className="rounded bg-surface-800 px-1 text-surface-300">
                .\scripts\setup-dev.ps1
              </code>
            </p>
            <p className="mt-1 text-[10px] text-warning-200">
              Regla de bloqueo: GitGov Desktop no permite Commit ni Push hasta que la identidad local del repo esté completa y alineada con tu cuenta autenticada.
            </p>
          </div>
        </div>
      )}
      <div className="flex gap-4">
        <div className="flex-1 space-y-2">
          <div className="flex gap-2">
            <select
              value={commitType}
              onChange={(e) => setCommitType(e.target.value)}
              className="px-2.5 py-2 bg-surface-800 border border-surface-700/50 rounded-lg text-white text-xs focus:outline-none focus:border-brand-500/50 transition-colors"
            >
              {COMMIT_TYPES.map((type) => (
                <option key={type.value} value={type.value}>
                  {type.label}
                </option>
              ))}
            </select>
            <input
              type="text"
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && hasStagedFiles && isValidMessage && !isIdentityBlocked) {
                  handleCommit()
                }
              }}
              placeholder="descripción del cambio"
              className="flex-1 px-3 py-2 bg-surface-800 border border-surface-700/50 rounded-lg text-white text-xs placeholder-surface-600 focus:outline-none focus:border-brand-500/50 transition-colors"
            />
          </div>

          {branchSync && currentBranch && (
            <div className="flex flex-wrap items-center gap-1.5 text-[11px]">
              {!hasUpstream && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-warning-500/30 bg-warning-500/10 text-warning-300">
                  <AlertTriangle size={11} strokeWidth={1.75} />
                  La rama no tiene upstream remoto configurado
                </span>
              )}

              {hasUpstream && ahead > 0 && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-danger-500/30 bg-danger-500/10 text-danger-300">
                  <ArrowUp size={11} strokeWidth={1.75} />
                  {ahead} commit(s) sin push
                </span>
              )}

              {hasUpstream && behind > 0 && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-warning-500/30 bg-warning-500/10 text-warning-300">
                  <ArrowDown size={11} strokeWidth={1.75} />
                  {behind} commit(s) pendientes de pull
                </span>
              )}
            </div>
          )}

          <div className="flex items-center gap-2 text-[11px] px-0.5">
            <span className="text-surface-600">Preview:</span>
            <code className={clsx(
              'px-1.5 py-0.5 rounded mono-data text-[11px] transition-colors',
              isValidMessage
                ? 'bg-success-500/10 text-success-400'
                : 'bg-surface-800/50 text-surface-600'
            )}>
              {fullMessage || 'mensaje vacío'}
            </code>
          </div>
        </div>

        <div className="flex flex-col gap-2 justify-center">
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="ghost"
              onClick={handleUnstageAll}
              disabled={!hasStagedFiles}
              title="Limpiar staging"
            >
              <RotateCcw size={13} strokeWidth={1.5} />
            </Button>

            <Button
              size="sm"
              onClick={handleCommit}
              loading={isCommitting}
              disabled={!hasStagedFiles || !isValidMessage || isIdentityBlocked}
            >
              <GitCommit size={13} strokeWidth={1.5} />
              Commit ({stagedFiles.size})
            </Button>
          </div>

          <Button
            size="sm"
            variant="outline"
            onClick={handlePush}
            loading={isPushing}
            disabled={!canPush || isIdentityBlocked}
          >
            <Upload size={13} strokeWidth={1.5} />
            Push
          </Button>
        </div>
      </div>
    </div>
  )
}
