import { useState, useMemo } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import type { BranchInfo } from '@/lib/types'
import { Button } from '@/components/shared/Button'
import { Modal } from '@/components/shared/Modal'
import { ChevronDown, GitBranch, Plus, Check, Search } from 'lucide-react'
import clsx from 'clsx'
import { BranchCreator } from './BranchCreator'

interface BranchSelectorProps {
  userLogin: string
  isAdmin: boolean
  userGroup?: string
}

export function BranchSelector({ userLogin, isAdmin, userGroup }: BranchSelectorProps) {
  const { branches, currentBranch, checkoutBranch } = useRepoStore()
  const [isOpen, setIsOpen] = useState(false)
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [search, setSearch] = useState('')
  const [switching, setSwitching] = useState<string | null>(null)

  const filteredBranches = useMemo(() => {
    if (!search) return branches
    return branches.filter((b) =>
      b.name.toLowerCase().includes(search.toLowerCase())
    )
  }, [branches, search])

  const localBranches = filteredBranches.filter((b) => !b.is_remote)
  const remoteBranches = filteredBranches.filter((b) => b.is_remote)

  const handleCheckout = async (branch: BranchInfo) => {
    if (branch.is_current || branch.is_remote) return
    setSwitching(branch.name)
    try {
      await checkoutBranch(branch.name)
      setIsOpen(false)
    } catch {
      // Error handled by store
    } finally {
      setSwitching(null)
    }
  }

  return (
    <>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-3 py-2 bg-surface-800 hover:bg-surface-700 border border-surface-700 rounded-lg transition-colors"
      >
        <GitBranch size={16} className="text-brand-500" />
        <span className="text-sm text-white truncate max-w-[200px]">
          {currentBranch || 'Sin rama'}
        </span>
        <ChevronDown size={16} className="text-surface-400" />
      </button>

      {isOpen && (
        <div className="absolute top-full left-0 mt-1 w-72 bg-surface-800 border border-surface-700 rounded-lg shadow-xl z-50">
          <div className="p-2 border-b border-surface-700">
            <div className="relative">
              <Search size={14} className="absolute left-2 top-1/2 -translate-y-1/2 text-surface-500" />
              <input
                type="text"
                placeholder="Buscar rama..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="w-full pl-7 pr-2 py-1.5 bg-surface-900 border border-surface-700 rounded text-sm text-white placeholder-surface-500 focus:outline-none focus:border-brand-500"
              />
            </div>
          </div>

          <div className="max-h-64 overflow-y-auto">
            {localBranches.length > 0 && (
              <div>
                <p className="px-3 py-1 text-xs text-surface-500 font-medium">Locales</p>
                {localBranches.map((branch) => (
                  <BranchItem
                    key={branch.name}
                    branch={branch}
                    isLoading={switching === branch.name}
                    onCheckout={() => handleCheckout(branch)}
                  />
                ))}
              </div>
            )}

            {remoteBranches.length > 0 && (
              <div>
                <p className="px-3 py-1 text-xs text-surface-500 font-medium">Remotas</p>
                {remoteBranches.map((branch) => (
                  <BranchItem
                    key={branch.name}
                    branch={branch}
                    isLoading={false}
                    disabled
                  />
                ))}
              </div>
            )}
          </div>

          <div className="p-2 border-t border-surface-700">
            <Button
              variant="ghost"
              size="sm"
              className="w-full"
              onClick={() => {
                setShowCreateModal(true)
                setIsOpen(false)
              }}
            >
              <Plus size={14} className="mr-1" />
              Nueva rama
            </Button>
          </div>
        </div>
      )}

      <Modal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        title="Crear nueva rama"
        size="md"
      >
        <BranchCreator
          userLogin={userLogin}
          isAdmin={isAdmin}
          userGroup={userGroup}
          onSuccess={() => setShowCreateModal(false)}
        />
      </Modal>
    </>
  )
}

interface BranchItemProps {
  branch: BranchInfo
  isLoading: boolean
  disabled?: boolean
  onCheckout?: () => void
}

function BranchItem({ branch, isLoading, disabled, onCheckout }: BranchItemProps) {
  return (
    <button
      onClick={onCheckout}
      disabled={disabled || isLoading || branch.is_current}
      className={clsx(
        'w-full flex items-center gap-2 px-3 py-2 text-left text-sm hover:bg-surface-700 transition-colors',
        disabled && 'opacity-50 cursor-not-allowed',
        branch.is_current && 'bg-surface-700'
      )}
    >
      <span className="w-4">
        {branch.is_current && <Check size={14} className="text-success-500" />}
        {isLoading && <span className="animate-spin">⏳</span>}
      </span>
      <span className="flex-1 truncate text-white">{branch.name}</span>
      {branch.last_commit_hash && (
        <span className="text-xs text-surface-500 font-mono">{branch.last_commit_hash}</span>
      )}
    </button>
  )
}
