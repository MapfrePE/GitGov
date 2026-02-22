import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { Header } from '@/components/layout/Header'
import { Button } from '@/components/shared/Button'
import { Modal } from '@/components/shared/Modal'

export function SettingsPage() {
  const { user, logout } = useAuthStore()
  const { repoPath, config } = useRepoStore()
  const [showRepoSelector, setShowRepoSelector] = useState(false)

  return (
    <div className="h-full flex flex-col">
      <Header />
      
      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl mx-auto space-y-6">
          <section className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Sesión</h2>
            {user && (
              <div className="space-y-3">
                <div className="flex items-center gap-3">
                  <img
                    src={user.avatar_url}
                    alt={user.login}
                    className="w-12 h-12 rounded-full"
                  />
                  <div>
                    <p className="text-white font-medium">{user.name}</p>
                    <p className="text-surface-400 text-sm">@{user.login}</p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {user.is_admin && (
                    <span className="text-xs bg-brand-500/20 text-brand-400 px-2 py-1 rounded">
                      Administrador
                    </span>
                  )}
                  {user.group && (
                    <span className="text-xs bg-surface-600 text-surface-300 px-2 py-1 rounded">
                      Grupo: {user.group}
                    </span>
                  )}
                </div>
                <Button variant="danger" size="sm" onClick={logout}>
                  Cerrar sesión
                </Button>
              </div>
            )}
          </section>

          <section className="card">
            <h2 className="text-lg font-semibold text-white mb-4">Repositorio</h2>
            <div className="space-y-3">
              <div>
                <p className="text-sm text-surface-400 mb-1">Ruta actual</p>
                <p className="text-white font-mono text-sm bg-surface-900 p-2 rounded">
                  {repoPath || 'No seleccionado'}
                </p>
              </div>
              <Button variant="secondary" onClick={() => setShowRepoSelector(true)}>
                Cambiar repositorio
              </Button>
            </div>
          </section>

          {config && (
            <section className="card">
              <h2 className="text-lg font-semibold text-white mb-4">Configuración GitGov</h2>
              <div className="bg-surface-900 rounded-lg p-4">
                <pre className="text-xs text-surface-300 font-mono overflow-auto whitespace-pre-wrap">
                  {JSON.stringify(config, null, 2)}
                </pre>
              </div>
            </section>
          )}
        </div>
      </div>

      <Modal
        isOpen={showRepoSelector}
        onClose={() => setShowRepoSelector(false)}
        title="Cambiar repositorio"
        size="lg"
      >
        <div className="text-center py-4">
          <p className="text-surface-400 mb-4">
            Selecciona un nuevo repositorio para gestionar
          </p>
          <Button onClick={() => setShowRepoSelector(false)}>
            Seleccionar desde el selector principal
          </Button>
        </div>
      </Modal>
    </div>
  )
}
