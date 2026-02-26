import type { ReactNode } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { Sidebar } from './Sidebar'
import { LoginScreen } from '@/components/auth/LoginScreen'
import { RepoSelector } from '@/components/repo/RepoSelector'
import { Skeleton, SkeletonFileRow } from '@/components/shared/Skeleton'

interface MainLayoutProps {
  children: ReactNode
}

export function MainLayout({ children }: MainLayoutProps) {
  const { user, authStep, isLoading } = useAuthStore()
  const { repoPath } = useRepoStore()

  if (isLoading) {
    return (
      <div className="flex h-screen bg-surface-900">
        <div className="w-14 bg-surface-950 border-r border-white/4 flex flex-col items-center py-3">
          <Skeleton className="w-8 h-8 rounded-lg mb-6" />
          <div className="flex-1 flex flex-col items-center gap-2">
            <Skeleton className="w-9 h-9 rounded-lg" />
            <Skeleton className="w-9 h-9 rounded-lg" />
            <Skeleton className="w-9 h-9 rounded-lg" />
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
