import type { ReactNode } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { Sidebar } from './Sidebar'
import { LoginScreen } from '@/components/auth/LoginScreen'
import { RepoSelector } from '@/components/repo/RepoSelector'

interface MainLayoutProps {
  children: ReactNode
}

export function MainLayout({ children }: MainLayoutProps) {
  const { user, authStep, isLoading } = useAuthStore()
  const { repoPath } = useRepoStore()

  if (isLoading) {
    return (
      <div className="min-h-screen bg-surface-900 flex items-center justify-center">
        <div className="text-center">
          <div className="w-12 h-12 border-4 border-brand-500 border-t-transparent rounded-full animate-spin mx-auto mb-4" />
          <p className="text-surface-400">Cargando...</p>
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
