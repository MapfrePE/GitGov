import { type ReactNode, useEffect } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { BranchSelector } from '@/components/branch/BranchSelector'
import { AlertTriangle, ArrowDown, ArrowUp, RefreshCw, FolderOpen } from 'lucide-react'
import { Button } from '@/components/shared/Button'
import clsx from 'clsx'

interface HeaderProps {
  children?: ReactNode
}

export function Header({ children }: HeaderProps) {
  const { user } = useAuthStore()
  const {
    repoPath,
    currentBranch,
    branchSync,
    refreshStatus,
    refreshBranches,
    refreshBranchSync,
    isLoadingStatus,
  } = useRepoStore()
  const isConnected = useControlPlaneStore((s) => s.isConnected)
  const connectionStatus = useControlPlaneStore((s) => s.connectionStatus)
  const checkConnection = useControlPlaneStore((s) => s.checkConnection)

  useEffect(() => {
    const interval = setInterval(() => {
      void checkConnection({ background: true })
    }, 30000)
    return () => clearInterval(interval)
  }, [checkConnection])

  const handleRefresh = async () => {
    await Promise.all([refreshStatus(), refreshBranches(), refreshBranchSync(), checkConnection()])
  }

  return (
    <header className="h-12 glass flex items-center justify-between px-5">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2 text-surface-400">
          <FolderOpen size={13} strokeWidth={1.5} />
          <span className="text-xs text-surface-300 font-medium truncate max-w-50">
            {repoPath?.split('/').pop() || 'Repositorio'}
          </span>
          <span
            title={
              connectionStatus === 'maintenance'
                ? 'Servidor en mantenimiento'
                : isConnected
                  ? 'Servidor conectado'
                  : 'Sin conexión al servidor'
            }
            className={clsx(
              'w-2 h-2 rounded-full shrink-0',
              connectionStatus === 'maintenance'
                ? 'bg-warning-500 animate-pulse'
                : isConnected
                  ? 'bg-success-500'
                  : 'bg-danger-500 animate-pulse'
            )}
          />
        </div>

        {currentBranch && user && (
          <>
            <div className="w-px h-4 bg-surface-700/50" />
            <BranchSelector
              userLogin={user.login}
              isAdmin={user.is_admin}
              userGroup={user.group}
            />
            {branchSync && (
              <div className="flex items-center gap-1">
                {!branchSync.has_upstream && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] border border-warning-500/30 bg-warning-500/10 text-warning-300">
                    <AlertTriangle size={10} strokeWidth={2} />
                    sin upstream
                  </span>
                )}
                {branchSync.ahead > 0 && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] border border-danger-500/30 bg-danger-500/10 text-danger-300">
                    <ArrowUp size={10} strokeWidth={2} />
                    {branchSync.ahead}
                  </span>
                )}
                {branchSync.behind > 0 && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] border border-warning-500/30 bg-warning-500/10 text-warning-300">
                    <ArrowDown size={10} strokeWidth={2} />
                    {branchSync.behind}
                  </span>
                )}
              </div>
            )}
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
