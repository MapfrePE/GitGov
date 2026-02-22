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
    <header className="h-14 bg-surface-800 border-b border-surface-700 flex items-center justify-between px-4">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <FolderOpen size={16} className="text-surface-400" />
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

      <div className="flex items-center gap-4">
        {children}
        
        <Button
          variant="ghost"
          size="sm"
          onClick={handleRefresh}
          loading={isLoadingStatus}
        >
          <RefreshCw size={14} className="mr-1" />
          Actualizar
        </Button>

        {user && (
          <div className="flex items-center gap-2">
            <img
              src={user.avatar_url}
              alt={user.login}
              className="w-6 h-6 rounded-full"
            />
            <span className="text-sm text-white">{user.login}</span>
            {user.is_admin && (
              <span className="text-xs bg-brand-500/20 text-brand-400 px-1.5 py-0.5 rounded">
                Admin
              </span>
            )}
          </div>
        )}
      </div>
    </header>
  )
}
