import { useRepoStore } from '@/store/useRepoStore'
import { DiffViewer } from '@/components/diff/DiffViewer'
import { TerminalPanel } from '@/components/cli/TerminalPanel'
import { PipelineVisualizer } from '@/components/cli/PipelineVisualizer'
import { AuditTrailPanel } from '@/components/cli/AuditTrailPanel'

/**
 * Workspace panel that occupies the main area of DashboardPage (above CommitPanel).
 *
 * When a file is selected → shows the diff viewer (existing behavior).
 * When no file is selected → Session Flow (top) + Terminal/Audit split (bottom).
 */
export function WorkspacePanel() {
  const activeDiffFile = useRepoStore((s) => s.activeDiffFile)

  // File selected → show diff (wrapped in flex-1 to not push CommitPanel off-screen)
  if (activeDiffFile) {
    return (
      <div className="flex-1 min-h-0 overflow-hidden">
        <DiffViewer />
      </div>
    )
  }

  // No file selected → Session Flow on top, Terminal + Audit Trail below
  return (
    <div className="flex flex-col flex-1 min-w-0 min-h-0 overflow-hidden">
      {/* Session Flow */}
      <div className="shrink-0 basis-[34%] min-h-[170px] max-h-[45%] overflow-hidden border-b border-surface-800">
        <PipelineVisualizer />
      </div>

      {/* Bottom workspace: terminal + audit trail */}
      <div className="grid flex-1 min-h-0 min-w-0 grid-cols-1 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <div className="min-h-0 min-w-0 border-b border-surface-800 xl:border-b-0 xl:border-r">
          <TerminalPanel />
        </div>
        <div className="min-h-[180px] min-w-0 xl:min-h-0">
          <AuditTrailPanel />
        </div>
      </div>
    </div>
  )
}
