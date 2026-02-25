import { useEffect } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import { Header } from '@/components/layout/Header'
import { FileList } from '@/components/diff/FileList'
import { DiffViewer } from '@/components/diff/DiffViewer'
import { CommitPanel } from '@/components/commit/CommitPanel'
import { Button } from '@/components/shared/Button'
import { FolderSync } from 'lucide-react'

export function DashboardPage() {
  const { repoPath, refreshStatus, refreshBranches, setRepoPath } = useRepoStore()

  useEffect(() => {
    if (repoPath) {
      refreshStatus()
      refreshBranches()
    }
  }, [repoPath, refreshStatus, refreshBranches])

  useEffect(() => {
    const interval = setInterval(() => {
      if (repoPath) {
        refreshStatus()
      }
    }, 30000)
    return () => clearInterval(interval)
  }, [repoPath, refreshStatus])

  const handleChangeRepo = () => {
    setRepoPath('')
  }

  return (
    <div className="h-full flex flex-col bg-surface-950">
      <Header>
        <Button variant="ghost" size="sm" onClick={handleChangeRepo}>
          <FolderSync size={14} />
          Cambiar repo
        </Button>
      </Header>

      <div className="flex-1 flex overflow-hidden">
        <div className="w-80 flex flex-col">
          <FileList />
        </div>

        <div className="flex-1 flex flex-col bg-surface-900">
          <DiffViewer />
          <CommitPanel />
        </div>
      </div>
    </div>
  )
}
