import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { Header } from '@/components/layout/Header'
import { Button } from '@/components/shared/Button'
import { Modal } from '@/components/shared/Modal'
import { User, FolderOpen, FileCode, LogOut, Shield, Users } from 'lucide-react'

export function SettingsPage() {
  const { user, logout } = useAuthStore()
  const { repoPath, config } = useRepoStore()
  const [showRepoSelector, setShowRepoSelector] = useState(false)

  return (
    <div className="h-full flex flex-col bg-surface-950">
      <Header />

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl mx-auto space-y-5 animate-fade-in">
          <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
            <div className="card-header mb-5">
              <User size={12} strokeWidth={1.5} />
              Sesión
            </div>
            {user && (
              <div className="space-y-4">
                <div className="flex items-center gap-4">
                  <img
                    src={user.avatar_url}
                    alt={user.login}
                    className="w-12 h-12 rounded-full ring-2 ring-surface-700 ring-offset-2 ring-offset-surface-800"
                  />
                  <div>
                    <p className="text-white font-semibold tracking-tight">{user.name}</p>
                    <p className="text-surface-500 text-xs">@{user.login}</p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {user.is_admin && (
                    <span className="text-[10px] font-medium bg-brand-500/10 text-brand-400 px-2 py-0.5 rounded inline-flex items-center gap-1">
                      <Shield size={9} />
                      Administrador
                    </span>
                  )}
                  {user.group && (
                    <span className="text-[10px] font-medium bg-surface-700/40 text-surface-400 px-2 py-0.5 rounded inline-flex items-center gap-1">
                      <Users size={9} />
                      {user.group}
                    </span>
                  )}
                </div>
                <Button variant="danger" size="sm" onClick={logout}>
                  <LogOut size={13} strokeWidth={1.5} />
                  Cerrar sesión
                </Button>
              </div>
            )}
          </section>

          <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
            <div className="card-header mb-5">
              <FolderOpen size={12} strokeWidth={1.5} />
              Repositorio
            </div>
            <div className="space-y-3">
              <div>
                <p className="text-[10px] text-surface-500 uppercase tracking-widest mb-1.5 font-medium">Ruta actual</p>
                <p className="text-white mono-data text-xs bg-surface-900/60 p-3 rounded-lg border border-surface-700/30">
                  {repoPath || 'No seleccionado'}
                </p>
              </div>
              <Button variant="secondary" onClick={() => setShowRepoSelector(true)}>
                <FolderOpen size={13} strokeWidth={1.5} />
                Cambiar repositorio
              </Button>
            </div>
          </section>

          {config && (
            <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
              <div className="card-header mb-5">
                <FileCode size={12} strokeWidth={1.5} />
                Configuración GitGov
              </div>
              <div className="bg-surface-900/60 rounded-lg p-4 border border-surface-700/30">
                <pre className="text-[11px] mono-data overflow-auto whitespace-pre-wrap leading-relaxed">
                  {JSON.stringify(config, null, 2).split('\n').map((line, i) => {
                    const keyMatch = line.match(/^(\s*)"([^"]+)"(:)/)
                    if (keyMatch) {
                      return (
                        <span key={i}>
                          {keyMatch[1]}<span className="text-brand-400">"{keyMatch[2]}"</span>{keyMatch[3]}
                          <span className="text-surface-400">{line.slice(keyMatch[0].length)}</span>{'\n'}
                        </span>
                      )
                    }
                    return <span key={i} className="text-surface-400">{line}{'\n'}</span>
                  })}
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
          <p className="text-surface-400 text-sm mb-4">
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
