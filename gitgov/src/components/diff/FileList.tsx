import { memo } from 'react'
import clsx from 'clsx'
import { useRepoStore } from '@/store/useRepoStore'
import { useAuthStore } from '@/store/useAuthStore'
import type { FileChange } from '@/lib/types'
import { FILE_STATUS_COLORS } from '@/lib/constants'
import { FileText, RefreshCw, AlertCircle, CheckSquare, Plus } from 'lucide-react'

interface FileItemProps {
  file: FileChange
  selected: boolean
  disabled: boolean
  onToggle: () => void
  onViewDiff: () => void
}

const FileItem = memo(function FileItem({ file, selected, disabled, onToggle, onViewDiff }: FileItemProps) {
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
        'flex items-center gap-2 px-3 py-2 hover:bg-surface-700/50 cursor-pointer group',
        disabled && 'opacity-50'
      )}
    >
      <button
        onClick={onToggle}
        disabled={disabled}
        className="flex-shrink-0"
      >
        <CheckSquare
          size={16}
          className={clsx(
            selected ? 'text-brand-500' : 'text-surface-500',
            'hover:text-brand-400'
          )}
        />
      </button>

      <span
        className={clsx(
          'flex-shrink-0 w-4 text-xs font-mono',
          FILE_STATUS_COLORS[statusChar ?? '?']
        )}
      >
        {statusChar}
      </span>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <span className="text-sm text-surface-400 truncate">
            {file.path.split('/').slice(0, -1).join('/')}/
          </span>
          <span className="text-sm text-white truncate font-medium">
            {file.path.split('/').pop()}
          </span>
        </div>
      </div>

      {file.staged && (
        <span className="text-xs bg-brand-500/20 text-brand-400 px-1.5 py-0.5 rounded">
          Preparado
        </span>
      )}

      {disabled && (
        <div className="relative group">
          <AlertCircle size={14} className="text-warning-500" />
          <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 bg-surface-900 text-xs text-white rounded opacity-0 group-hover:opacity-100 whitespace-nowrap">
            Path no permitido
          </span>
        </div>
      )}

      <button
        onClick={onViewDiff}
        className="opacity-0 group-hover:opacity-100 text-surface-400 hover:text-white transition-opacity"
      >
        <FileText size={14} />
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
  } = useRepoStore()

  const { user } = useAuthStore()

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

  const allSelected = fileChanges.length > 0 && selectedFiles.size === fileChanges.length

  return (
    <div className="h-full flex flex-col bg-surface-800 border-r border-surface-700">
      <div className="flex items-center justify-between px-3 py-2 border-b border-surface-700">
        <h3 className="text-sm font-medium text-white">
          Cambios ({fileChanges.length})
        </h3>
        <div className="flex gap-2">
          {selectedFiles.size > 0 && (
            <button
              onClick={handleStageSelected}
              className="text-xs text-brand-400 hover:text-brand-300 flex items-center gap-1"
            >
              <Plus size={12} />
              Preparar ({selectedFiles.size})
            </button>
          )}
          <button
            onClick={allSelected ? deselectAll : selectAll}
            className="text-xs text-surface-400 hover:text-white"
          >
            {allSelected ? 'Deseleccionar todo' : 'Seleccionar todo'}
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {fileChanges.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-surface-500 p-4">
            <RefreshCw size={24} className="mb-2" />
            <p className="text-sm">No hay cambios</p>
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
            />
          ))
        )}
      </div>

      {stagedFiles.size > 0 && (
        <div className="px-3 py-2 border-t border-surface-700 bg-surface-700/50">
          <p className="text-xs text-surface-400">
            {stagedFiles.size} archivo{stagedFiles.size !== 1 ? 's' : ''} en staging
          </p>
        </div>
      )}
    </div>
  )
}
