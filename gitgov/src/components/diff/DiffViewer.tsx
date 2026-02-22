import { useMemo } from 'react'
import { parseDiff, Diff, Hunk } from 'react-diff-view'
import 'react-diff-view/style/index.css'
import { useRepoStore } from '@/store/useRepoStore'
import { Spinner } from '@/components/shared/Spinner'
import { FileCode } from 'lucide-react'

export function DiffViewer() {
  const { activeDiffFile, activeDiff, isLoadingStatus } = useRepoStore()

  const files = useMemo(() => {
    if (!activeDiff) return []
    try {
      return parseDiff(activeDiff, { nearbySequences: 'zip' })
    } catch {
      return []
    }
  }, [activeDiff])

  if (!activeDiffFile) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-surface-500">
        <FileCode size={48} className="mb-4" />
        <p>Selecciona un archivo para ver el diff</p>
      </div>
    )
  }

  if (isLoadingStatus && !activeDiff) {
    return (
      <div className="h-full flex items-center justify-center">
        <Spinner size="lg" />
      </div>
    )
  }

  return (
    <div className="h-full overflow-auto bg-surface-900">
      <div className="sticky top-0 bg-surface-800 border-b border-surface-700 px-4 py-2">
        <h3 className="text-sm font-medium text-white truncate">{activeDiffFile}</h3>
      </div>

      {files.length === 0 ? (
        <div className="p-4 text-surface-500 text-sm">
          {activeDiff || 'No hay diff disponible'}
        </div>
      ) : (
        <div className="diff-view">
          {files.map((file, i) => (
            <Diff
              key={`${file.oldPath}-${file.newPath}-${i}`}
              viewType="unified"
              diffType={file.type}
              hunks={file.hunks}
            >
              {(hunks) =>
                hunks.map((hunk) => <Hunk key={hunk.content} hunk={hunk} />)
              }
            </Diff>
          ))}
        </div>
      )}
    </div>
  )
}
