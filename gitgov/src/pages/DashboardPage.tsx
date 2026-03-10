import { useEffect } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import { Header } from '@/components/layout/Header'
import { FileList } from '@/components/diff/FileList'
import { WorkspacePanel } from '@/components/cli/WorkspacePanel'
import { CommitPanel } from '@/components/commit/CommitPanel'
import { Button } from '@/components/shared/Button'
import { FolderSync } from 'lucide-react'

export function DashboardPage() {
  const { repoPath, refreshStatus, refreshBranches, refreshBranchSync, beginRepoSwitch } = useRepoStore()

  useEffect(() => {
    if (repoPath) {
      refreshStatus()
      refreshBranches()
      refreshBranchSync()
    }
  }, [repoPath, refreshStatus, refreshBranches, refreshBranchSync])

  // Repo status polling: 2 min cadence (local git state changes infrequently).
  // Server dashboard has its own 30 s poll; Header has its own 30 s connection check.
  useEffect(() => {
    const interval = setInterval(() => {
      if (repoPath) {
        refreshStatus()
        refreshBranchSync()
      }
    }, 120_000)
    return () => clearInterval(interval)
  }, [repoPath, refreshStatus, refreshBranchSync])

  const handleChangeRepo = () => {
    beginRepoSwitch()
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

        <div className="flex-1 min-w-0 min-h-0 flex flex-col bg-surface-900">
          <WorkspacePanel />
          <CommitPanel />
        </div>
      </div>
    </div>
  )
}
