import { memo, useState } from 'react'
import clsx from 'clsx'
import { useRepoStore } from '@/store/useRepoStore'
import { useAuthStore } from '@/store/useAuthStore'
import type { FileChange } from '@/lib/types'
import { FILE_STATUS_COLORS } from '@/lib/constants'
import { FileText, AlertCircle, CheckSquare, Plus, FileCode, Loader2 } from 'lucide-react'

interface FileItemProps {
  file: FileChange
  selected: boolean
  disabled: boolean
  onToggle: () => void
  onViewDiff: () => void
  onUnstage: () => void
}

const FileItem = memo(function FileItem({ file, selected, disabled, onToggle, onViewDiff, onUnstage }: FileItemProps) {
  const statusChar = {
    Modified: 'M',
    Added: 'A',
    Deleted: 'D',
    Renamed: 'R',
    Untracked: '?',
  }[file.status]

  return (
    <div
      className={clsx(
        'flex items-center gap-2.5 px-3 py-2 cursor-pointer group transition-colors duration-150',
        selected ? 'bg-white/[0.03]' : 'hover:bg-white/[0.02]',
        disabled && 'opacity-50'
      )}
    >
      <button
        onClick={onToggle}
        disabled={disabled}
        className="flex-shrink-0"
      >
        <CheckSquare
          size={14}
          strokeWidth={1.5}
          className={clsx(
            'transition-colors',
            selected ? 'text-brand-400' : 'text-surface-600',
            'hover:text-brand-400'
          )}
        />
      </button>

      <span
        className={clsx(
          'flex-shrink-0 w-4 h-4 rounded text-[9px] font-semibold mono-data flex items-center justify-center',
          FILE_STATUS_COLORS[statusChar ?? '?']
        )}
      >
        {statusChar}
      </span>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <span className="text-[11px] text-surface-600 truncate">
            {file.path.split('/').slice(0, -1).join('/')}/
          </span>
          <span className="text-xs text-surface-200 truncate font-medium">
            {file.path.split('/').pop()}
          </span>
        </div>
      </div>

      {file.staged && (
        <button
          onClick={(e) => { e.stopPropagation(); onUnstage() }}
          title="Quitar del staging"
          className="text-[9px] font-medium bg-brand-500/10 text-brand-400 px-1.5 py-0.5 rounded hover:bg-danger-500/15 hover:text-danger-400 transition-colors"
        >
          Staged ×
        </button>
      )}

      {disabled && (
        <div className="relative group">
          <AlertCircle size={12} strokeWidth={1.5} className="text-warning-500" />
          <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 bg-surface-900 text-[10px] text-white rounded opacity-0 group-hover:opacity-100 whitespace-nowrap">
            Path no permitido
          </span>
        </div>
      )}

      <button
        onClick={onViewDiff}
        className="opacity-0 group-hover:opacity-100 text-surface-500 hover:text-surface-300 transition-all duration-150"
      >
        <FileText size={13} strokeWidth={1.5} />
      </button>
    </div>
  )
})

export function FileList() {
  const {
    fileChanges,
    selectedFiles,
    stagedFiles,
    selectFile,
    deselectFile,
    selectAll,
    deselectAll,
    loadDiff,
    stageSelected,
    unstageFiles,
  } = useRepoStore()

  const { user } = useAuthStore()
  const [isPreparing, setIsPreparing] = useState(false)

  const handleToggle = (path: string, isSelected: boolean) => {
    if (isSelected) {
      deselectFile(path)
    } else {
      selectFile(path)
    }
  }

  const handleStageSelected = async () => {
    if (selectedFiles.size > 0 && user) {
      await stageSelected(user.login)
    }
  }

  const handlePrepareAll = async () => {
    if (!user) return
    setIsPreparing(true)
    try {
      selectAll()
      await stageSelected(user.login)
    } finally {
      setIsPreparing(false)
    }
  }

  const hasUnstagedFiles = fileChanges.some((f) => !f.staged)
  const someSelected = selectedFiles.size > 0

  return (
    <div className="h-full flex flex-col bg-surface-900/50 border-r border-surface-700/30">
      <div className="flex items-center justify-between px-4 py-3 border-b border-surface-700/30">
        <h3 className="text-[10px] font-medium text-surface-500 uppercase tracking-widest">
          Cambios ({fileChanges.length})
        </h3>
        <div className="flex gap-2">
          {someSelected ? (
            <>
              <button
                onClick={handleStageSelected}
                className="text-[11px] text-brand-400 hover:text-brand-300 flex items-center gap-1 transition-colors"
              >
                <Plus size={11} />
                Preparar ({selectedFiles.size})
              </button>
              <button
                onClick={deselectAll}
                className="text-[11px] text-surface-500 hover:text-surface-300 transition-colors"
              >
                Deseleccionar
              </button>
            </>
          ) : hasUnstagedFiles ? (
            <>
              <button
                onClick={handlePrepareAll}
                disabled={isPreparing}
                className="text-[11px] text-brand-400 hover:text-brand-300 flex items-center gap-1 transition-colors disabled:opacity-50"
              >
                {isPreparing ? (
                  <Loader2 size={11} className="animate-spin" />
                ) : (
                  <Plus size={11} />
                )}
                Preparar todo
              </button>
              <button
                onClick={selectAll}
                className="text-[11px] text-surface-500 hover:text-surface-300 transition-colors"
              >
                Seleccionar todo
              </button>
            </>
          ) : null}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto divide-y divide-surface-700/15">
        {fileChanges.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-surface-500 p-6">
            <FileCode size={24} strokeWidth={1.5} className="mb-3 text-surface-700" />
            <p className="text-xs font-medium text-surface-400">No hay cambios</p>
            <p className="text-[11px] text-surface-600 mt-1">Edita archivos para empezar</p>
          </div>
        ) : (
          fileChanges.map((file) => (
            <FileItem
              key={file.path}
              file={file}
              selected={selectedFiles.has(file.path)}
              disabled={false}
              onToggle={() => handleToggle(file.path, selectedFiles.has(file.path))}
              onViewDiff={() => loadDiff(file.path)}
              onUnstage={() => unstageFiles([file.path])}
            />
          ))
        )}
      </div>

      {stagedFiles.size > 0 && (
        <div className="px-4 py-2.5 border-t border-surface-700/30">
          <p className="text-[11px] text-brand-400 font-medium mono-data">
            {stagedFiles.size} archivo{stagedFiles.size !== 1 ? 's' : ''} en staging
          </p>
        </div>
      )}
    </div>
  )
}
