import { useState, useMemo } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import { useAuthStore } from '@/store/useAuthStore'
import { Button } from '@/components/shared/Button'
import { COMMIT_TYPES } from '@/lib/constants'
import { GitCommit, Upload, RotateCcw } from 'lucide-react'
import { toast } from '@/components/shared/Toast'
import clsx from 'clsx'

export function CommitPanel() {
  const { stagedFiles, fileChanges, currentBranch, commit, push, unstageAll, refreshStatus } = useRepoStore()
  const { user } = useAuthStore()
  const [message, setMessage] = useState('')
  const [commitType, setCommitType] = useState('feat')
  const [isCommitting, setIsCommitting] = useState(false)
  const [isPushing, setIsPushing] = useState(false)
  const [lastCommitHash, setLastCommitHash] = useState<string | null>(null)

  const fullMessage = useMemo(() => {
    if (!message.trim()) return ''
    if (message.includes(':')) return message
    return `${commitType}: ${message}`
  }, [commitType, message])

  const isValidMessage = useMemo(() => {
    if (!fullMessage) return false
    return /^(feat|fix|docs|style|refactor|test|chore|hotfix):/.test(fullMessage)
  }, [fullMessage])

  const hasStagedFiles = stagedFiles.size > 0
  const hasUncommittedChanges = fileChanges.some((f) => f.staged) || stagedFiles.size > 0

  const handleCommit = async () => {
    if (!user || !isValidMessage) return
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
    } catch (e) {
      toast('error', String(e))
    } finally {
      setIsCommitting(false)
    }
  }

  const handlePush = async () => {
    if (!user || !currentBranch) return
    setIsPushing(true)
    try {
      await push(currentBranch, user.login)
      toast('success', `Push exitoso a ${currentBranch}`)
      setLastCommitHash(null)
      await refreshStatus()
    } catch (e) {
      toast('error', String(e))
    } finally {
      setIsPushing(false)
    }
  }

  const handleUnstageAll = async () => {
    await unstageAll()
    toast('info', 'Staging area limpiado')
  }

  return (
    <div className="border-t border-surface-700 bg-surface-800 p-4">
      <div className="flex gap-4">
        <div className="flex-1">
          <div className="flex gap-2 mb-2">
            <select
              value={commitType}
              onChange={(e) => setCommitType(e.target.value)}
              className="px-2 py-2 bg-surface-900 border border-surface-700 rounded-lg text-white text-sm focus:outline-none focus:border-brand-500"
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
              placeholder="descripción del cambio"
              className="flex-1 px-3 py-2 bg-surface-900 border border-surface-700 rounded-lg text-white placeholder-surface-500 focus:outline-none focus:border-brand-500"
            />
          </div>
          
          <div className="flex items-center gap-2 text-xs">
            <span className="text-surface-500">Vista previa:</span>
            <code className={clsx(
              'px-2 py-0.5 rounded',
              isValidMessage ? 'bg-success-500/20 text-success-400' : 'bg-surface-700 text-surface-400'
            )}>
              {fullMessage || 'mensaje vacío'}
            </code>
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="ghost"
              onClick={handleUnstageAll}
              disabled={!hasStagedFiles}
              title="Limpiar staging"
            >
              <RotateCcw size={14} />
            </Button>
            
            <Button
              size="sm"
              onClick={handleCommit}
              loading={isCommitting}
              disabled={!hasStagedFiles || !isValidMessage}
            >
              <GitCommit size={14} className="mr-1" />
              Commit ({stagedFiles.size})
            </Button>
          </div>

          <Button
            size="sm"
            variant="secondary"
            onClick={handlePush}
            loading={isPushing}
            disabled={!lastCommitHash && !hasUncommittedChanges}
          >
            <Upload size={14} className="mr-1" />
            Push
          </Button>
        </div>
      </div>
    </div>
  )
}
