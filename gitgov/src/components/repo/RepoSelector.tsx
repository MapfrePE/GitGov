import { useState, useCallback } from 'react'
import { useRepoStore } from '@/store/useRepoStore'
import { Button } from '@/components/shared/Button'
import { Spinner } from '@/components/shared/Spinner'
import { FolderOpen, CheckCircle, XCircle, FolderGit2, ArrowLeft } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'

interface ValidationItemProps {
  label: string
  valid: boolean
}

function ValidationItem({ label, valid }: ValidationItemProps) {
  return (
    <div className="flex items-center gap-2 text-sm">
      {valid ? (
        <CheckCircle size={16} className="text-success-500" />
      ) : (
        <XCircle size={16} className="text-danger-500" />
      )}
      <span className={valid ? 'text-surface-300' : 'text-surface-500'}>{label}</span>
    </div>
  )
}

export function RepoSelector() {
  const {
    setRepoPath,
    cancelRepoSwitch,
    previousRepoPath,
    validation,
    isLoadingStatus,
    error,
  } = useRepoStore()
  const [selectedPath, setSelectedPath] = useState<string | null>(null)

  const handleSelectFolder = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Seleccionar repositorio',
      })
      if (selected && typeof selected === 'string') {
        setSelectedPath(selected)
        await setRepoPath(selected)
      }
    } catch (e) {
      console.error('Failed to select folder:', e)
    }
  }, [setRepoPath])

  return (
    <div className="min-h-screen bg-surface-900 flex items-center justify-center p-4">
      <div className="max-w-lg w-full">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-brand-600 mb-4">
            <FolderGit2 size={32} />
          </div>
          <h1 className="text-2xl font-bold text-white mb-2">Selecciona un repositorio</h1>
          <p className="text-surface-400">
            Elige la carpeta del repositorio Git que deseas gestionar
          </p>
        </div>

        <div className="card">
          {previousRepoPath && (
            <div className="mb-4 rounded-lg border border-surface-700 bg-surface-800/70 p-3">
              <p className="mb-2 text-xs text-surface-400">Repositorio anterior:</p>
              <p className="mb-3 truncate text-xs font-mono text-surface-300">{previousRepoPath}</p>
              <Button
                variant="secondary"
                className="w-full"
                onClick={() => {
                  void cancelRepoSwitch()
                }}
              >
                <ArrowLeft size={16} className="mr-2" />
                Volver al repo anterior
              </Button>
            </div>
          )}

          {error && (
            <div className="mb-4 p-3 bg-danger-500/20 border border-danger-500/50 rounded-lg text-danger-400 text-sm">
              {error}
            </div>
          )}

          <Button onClick={handleSelectFolder} className="w-full mb-4" size="lg">
            <FolderOpen size={20} className="mr-2" />
            Seleccionar carpeta
          </Button>

          {selectedPath && (
            <div className="bg-surface-700 rounded-lg p-3 mb-4">
              <p className="text-xs text-surface-400 mb-1">Ruta seleccionada:</p>
              <p className="text-sm text-white font-mono break-all">{selectedPath}</p>
            </div>
          )}

          {validation && (
            <div className="space-y-2">
              <p className="text-sm font-medium text-surface-300 mb-2">Validaciones:</p>
              <ValidationItem label="Ruta existe" valid={validation.path_exists} />
              <ValidationItem label="Es un repositorio Git" valid={validation.is_git_repo} />
              <ValidationItem label="Tiene remote origin" valid={validation.has_remote_origin} />
              <ValidationItem label="Tiene gitgov.toml" valid={validation.has_gitgov_toml} />
            </div>
          )}

          {isLoadingStatus && (
            <div className="flex items-center justify-center mt-4">
              <Spinner size="md" />
            </div>
          )}

          {validation && !validation.has_gitgov_toml && (
            <div className="mt-4 p-3 bg-warning-500/20 border border-warning-500/50 rounded-lg">
              <p className="text-warning-400 text-sm">
                El repositorio no tiene un archivo gitgov.toml. Las operaciones de push y creación
                de ramas estarán limitadas hasta que se agregue.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
