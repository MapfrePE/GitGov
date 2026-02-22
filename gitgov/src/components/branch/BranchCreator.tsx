import { useState, useEffect } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import { Button } from '@/components/shared/Button'
import { tauriInvoke } from '@/lib/tauri'
import type { ValidationResult } from '@/lib/types'
import { Check, X, AlertCircle } from 'lucide-react'

interface BranchCreatorProps {
  userLogin: string
  isAdmin: boolean
  userGroup?: string
  onSuccess: () => void
}

export function BranchCreator({ userLogin, isAdmin, userGroup, onSuccess }: BranchCreatorProps) {
  const { repoPath, currentBranch, branches, createBranch } = useRepoStore()
  const [name, setName] = useState('')
  const [fromBranch, setFromBranch] = useState(currentBranch || 'main')
  const [validation, setValidation] = useState<ValidationResult | null>(null)
  const [isCreating, setIsCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (currentBranch) {
      setFromBranch(currentBranch)
    }
  }, [currentBranch])

  useEffect(() => {
    if (!name || !repoPath) {
      setValidation(null)
      return
    }

    const validate = async () => {
      try {
        const result = await tauriInvoke<ValidationResult>('cmd_validate_branch_name', {
          name,
          repoPath,
          developerLogin: userLogin,
          isAdmin,
          userGroup: userGroup ?? null,
        })
        setValidation(result)
      } catch {
        setValidation(null)
      }
    }

    const timer = setTimeout(validate, 300)
    return () => clearTimeout(timer)
  }, [name, repoPath, userLogin, isAdmin, userGroup])

  const handleCreate = async () => {
    if (!name.trim()) return
    setIsCreating(true)
    setError(null)
    try {
      await createBranch(name.trim(), fromBranch, userLogin, isAdmin, userGroup)
      onSuccess()
    } catch (e) {
      setError(String(e))
    } finally {
      setIsCreating(false)
    }
  }

  const localBranches = branches.filter((b) => !b.is_remote)

  return (
    <div className="space-y-4">
      <div>
        <label className="block text-sm font-medium text-surface-300 mb-1">
          Nombre de la rama
        </label>
        <div className="relative">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="feat/TICKET-descripcion"
            className="w-full px-3 py-2 bg-surface-900 border border-surface-700 rounded-lg text-white placeholder-surface-500 focus:outline-none focus:border-brand-500 pr-8"
          />
          {validation && (
            <span className="absolute right-2 top-1/2 -translate-y-1/2">
              {validation.type === 'Valid' ? (
                <Check size={16} className="text-success-500" />
              ) : (
                <X size={16} className="text-danger-500" />
              )}
            </span>
          )}
        </div>
        {validation?.type === 'Blocked' && (
          <p className="mt-1 text-sm text-danger-400 flex items-center gap-1">
            <AlertCircle size={14} />
            {validation.message}
          </p>
        )}
      </div>

      <div>
        <label className="block text-sm font-medium text-surface-300 mb-1">
          Crear desde
        </label>
        <select
          value={fromBranch}
          onChange={(e) => setFromBranch(e.target.value)}
          className="w-full px-3 py-2 bg-surface-900 border border-surface-700 rounded-lg text-white focus:outline-none focus:border-brand-500"
        >
          {localBranches.map((b) => (
            <option key={b.name} value={b.name}>
              {b.name}
            </option>
          ))}
        </select>
      </div>

      {error && (
        <div className="p-3 bg-danger-500/20 border border-danger-500/50 rounded-lg text-danger-400 text-sm">
          {error}
        </div>
      )}

      <div className="flex justify-end gap-2">
        <Button
          variant="secondary"
          onClick={onSuccess}
        >
          Cancelar
        </Button>
        <Button
          onClick={handleCreate}
          loading={isCreating}
          disabled={!name.trim() || validation?.type === 'Blocked'}
        >
          Crear rama
        </Button>
      </div>
    </div>
  )
}
