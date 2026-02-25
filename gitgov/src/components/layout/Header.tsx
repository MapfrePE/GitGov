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
    <header className="h-12 glass flex items-center justify-between px-5">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2 text-surface-400">
          <FolderOpen size={13} strokeWidth={1.5} />
          <span className="text-xs text-surface-300 font-medium truncate max-w-[200px]">
            {repoPath?.split('/').pop() || 'Repositorio'}
          </span>
        </div>

        {currentBranch && user && (
          <>
            <div className="w-px h-4 bg-surface-700/50" />
            <BranchSelector
              userLogin={user.login}
              isAdmin={user.is_admin}
              userGroup={user.group}
            />
          </>
        )}
      </div>

      <div className="flex items-center gap-2">
        {children}

        <Button
          variant="ghost"
          size="sm"
          onClick={handleRefresh}
          loading={isLoadingStatus}
        >
          <RefreshCw size={13} strokeWidth={1.5} />
          Actualizar
        </Button>

        {user && (
          <>
            <div className="w-px h-4 bg-surface-700/50 mx-1" />
            <div className="flex items-center gap-2">
              <img
                src={user.avatar_url}
                alt={user.login}
                className="w-6 h-6 rounded-full"
              />
              <span className="text-xs text-surface-400 font-medium">{user.login}</span>
              {user.is_admin && (
                <span className="text-[10px] font-medium bg-brand-500/10 text-brand-400 px-1.5 py-0.5 rounded">
                  Admin
                </span>
              )}
            </div>
          </>
        )}
      </div>
    </header>
  )
}
