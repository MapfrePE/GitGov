import { useEffect, type ReactNode } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Sidebar } from './Sidebar'
import { LoginScreen } from '@/components/auth/LoginScreen'
import { PinUnlockScreen } from '@/components/auth/PinUnlockScreen'
import { ControlPlaneAuthScreen } from '@/components/auth/ControlPlaneAuthScreen'
import { RepoSelector } from '@/components/repo/RepoSelector'
import { Skeleton, SkeletonFileRow } from '@/components/shared/Skeleton'

interface MainLayoutProps {
  children: ReactNode
}

export function MainLayout({ children }: MainLayoutProps) {
  const { user, authStep, isLoading, isPinEnabled, pinUnlocked } = useAuthStore()
  const { repoPath } = useRepoStore()
  const serverConfig = useControlPlaneStore((s) => s.serverConfig)
  const isConnected = useControlPlaneStore((s) => s.isConnected)
  const userRole = useControlPlaneStore((s) => s.userRole)
  const userOrgId = useControlPlaneStore((s) => s.userOrgId)
  const selectedOrgName = useControlPlaneStore((s) => s.selectedOrgName)
  const disconnect = useControlPlaneStore((s) => s.disconnect)
  const resetControlPlaneAuthGate = useControlPlaneStore((s) => s.resetControlPlaneAuthGate)
  const refreshChatMessagesForActiveUser = useControlPlaneStore((s) => s.refreshChatMessagesForActiveUser)

  useEffect(() => {
    if (!user || authStep !== 'authenticated') {
      resetControlPlaneAuthGate()
      if (serverConfig || isConnected || userRole) {
        disconnect()
      }
    }
  }, [user, authStep, serverConfig, isConnected, userRole, disconnect, resetControlPlaneAuthGate])

  useEffect(() => {
    refreshChatMessagesForActiveUser()
  }, [user?.login, refreshChatMessagesForActiveUser])

  if (isLoading) {
    return (
      <div className="flex h-screen bg-surface-900">
        <div className="w-18 bg-surface-950 border-r border-white/4 flex flex-col items-center py-4">
          <Skeleton className="w-12 h-12 rounded-lg mb-7" />
          <div className="flex-1 flex flex-col items-center gap-2">
            <Skeleton className="w-10 h-10 rounded-lg" />
            <Skeleton className="w-10 h-10 rounded-lg" />
            <Skeleton className="w-10 h-10 rounded-lg" />
          </div>
        </div>
        <div className="flex-1 flex flex-col">
          <div className="h-12 border-b border-surface-700/30 flex items-center px-5 gap-4">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-7 w-40 rounded-lg" />
          </div>
          <div className="flex-1 flex">
            <div className="w-80 border-r border-surface-700/30 p-2 space-y-1">
              {[1, 2, 3, 4, 5, 6].map((i) => (
                <SkeletonFileRow key={i} />
              ))}
            </div>
            <div className="flex-1 p-6">
              <Skeleton className="h-4 w-48 mb-4" />
              <div className="space-y-2">
                <Skeleton className="h-3 w-full" />
                <Skeleton className="h-3 w-5/6" />
                <Skeleton className="h-3 w-4/6" />
              </div>
            </div>
          </div>
        </div>
      </div>
    )
  }

  if (!user || authStep !== 'authenticated') {
    return <LoginScreen />
  }

  if (isPinEnabled && !pinUnlocked) {
    return <PinUnlockScreen />
  }

  const requiresOrgNameForAdminScope = userRole === 'Admin' && Boolean(userOrgId) && !selectedOrgName.trim()
  const requiresControlPlaneAuth = Boolean(
    user &&
    authStep === 'authenticated' &&
    (!isConnected || !userRole || requiresOrgNameForAdminScope),
  )

  if (requiresControlPlaneAuth) {
    return <ControlPlaneAuthScreen />
  }

  if (!repoPath) {
    return <RepoSelector />
  }

  return (
    <div className="flex h-screen bg-surface-900">
      <Sidebar />
      <main className="flex-1 overflow-hidden">{children}</main>
    </div>
  )
}
