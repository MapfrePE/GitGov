import type { ReactNode } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { BranchSelector } from '@/components/branch/BranchSelector'
import { RefreshCw, FolderOpen } from 'lucide-react'
import { Button } from '@/components/shared/Button'

interface HeaderProps {
  children?: ReactNode
}

export function Header({ children }: HeaderProps) {
  const { user } = useAuthStore()
  const { repoPath, currentBranch, refreshStatus, refreshBranches, isLoadingStatus } = useRepoStore()

  const handleRefresh = async () => {
    await Promise.all([refreshStatus(), refreshBranches()])
  }

  return (
    <header className="h-13 glass border-b border-surface-700/50 flex items-center justify-between px-5">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2 px-2.5 py-1 rounded-lg bg-surface-800/60">
          <FolderOpen size={14} className="text-surface-400" />
          <span className="text-sm text-white font-medium truncate max-w-[200px]">
            {repoPath?.split('/').pop() || 'Repositorio'}
          </span>
        </div>

        {currentBranch && user && (
          <div className="relative">
            <BranchSelector
              userLogin={user.login}
              isAdmin={user.is_admin}
              userGroup={user.group}
            />
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        {children}

        <Button
          variant="ghost"
          size="sm"
          onClick={handleRefresh}
          loading={isLoadingStatus}
        >
          <RefreshCw size={14} />
          Actualizar
        </Button>

        {user && (
          <div className="flex items-center gap-2.5 pl-3 border-l border-surface-700/50">
            <img
              src={user.avatar_url}
              alt={user.login}
              className="w-7 h-7 rounded-full ring-2 ring-surface-600"
            />
            <span className="text-sm text-surface-300 font-medium">{user.login}</span>
            {user.is_admin && (
              <span className="text-[10px] font-semibold bg-brand-500/15 text-brand-400 px-2 py-0.5 rounded-full ring-1 ring-brand-500/20">
                Admin
              </span>
            )}
          </div>
        )}
      </div>
    </header>
  )
}
