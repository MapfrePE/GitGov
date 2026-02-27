import { useEffect, useState } from 'react'
import { Key, RefreshCw, ShieldOff } from 'lucide-react'
import { useControlPlaneStore, type ApiKeyInfo } from '@/store/useControlPlaneStore'

function formatTimestamp(ms: number | null | undefined): string {
  if (!ms) return '—'
  return new Date(ms).toLocaleDateString('es', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function roleBadgeClass(role: string): string {
  if (role === 'Admin') return 'bg-amber-500/20 text-amber-300 border-amber-500/30'
  if (role === 'Developer') return 'bg-blue-500/20 text-blue-300 border-blue-500/30'
  return 'bg-surface-700 text-surface-300 border-surface-600'
}

export function ApiKeyManagerWidget() {
  const { apiKeys, isLoadingApiKeys, loadApiKeys, revokeApiKey } = useControlPlaneStore()
  const [confirmingId, setConfirmingId] = useState<string | null>(null)
  const [revokingId, setRevokingId] = useState<string | null>(null)

  useEffect(() => {
    void loadApiKeys()
  }, [loadApiKeys])

  const handleRevoke = async (key: ApiKeyInfo) => {
    if (confirmingId !== key.id) {
      setConfirmingId(key.id)
      return
    }
    setRevokingId(key.id)
    setConfirmingId(null)
    await revokeApiKey(key.id)
    setRevokingId(null)
  }

  return (
    <div className="glass-panel p-5">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Key size={14} className="text-surface-400" />
          <span className="card-header">API Keys</span>
        </div>
        <button
          onClick={() => void loadApiKeys()}
          disabled={isLoadingApiKeys}
          className="p-1 rounded text-surface-500 hover:text-surface-300 transition-colors"
          title="Refrescar"
        >
          <RefreshCw size={13} className={isLoadingApiKeys ? 'animate-spin' : ''} />
        </button>
      </div>

      {isLoadingApiKeys && apiKeys.length === 0 ? (
        <div className="py-8 text-center text-surface-500 text-xs">Cargando...</div>
      ) : apiKeys.length === 0 ? (
        <div className="py-8 text-center text-surface-500 text-xs">Sin API keys registradas</div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-surface-700">
                <th className="text-left pb-2 text-surface-500 font-medium">Client ID</th>
                <th className="text-left pb-2 text-surface-500 font-medium">Rol</th>
                <th className="text-left pb-2 text-surface-500 font-medium">Creada</th>
                <th className="text-left pb-2 text-surface-500 font-medium">Último uso</th>
                <th className="text-left pb-2 text-surface-500 font-medium">Estado</th>
                <th className="text-right pb-2 text-surface-500 font-medium">Acción</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-surface-800">
              {apiKeys.map((key) => (
                <tr key={key.id} className="hover:bg-surface-800/30 transition-colors">
                  <td className="py-2 pr-3 font-mono text-surface-200 max-w-[120px] truncate" title={key.client_id}>
                    {key.client_id}
                  </td>
                  <td className="py-2 pr-3">
                    <span className={`inline-block px-1.5 py-0.5 rounded border text-[10px] font-medium ${roleBadgeClass(key.role)}`}>
                      {key.role}
                    </span>
                  </td>
                  <td className="py-2 pr-3 text-surface-400 whitespace-nowrap">
                    {formatTimestamp(key.created_at)}
                  </td>
                  <td className="py-2 pr-3 text-surface-400 whitespace-nowrap">
                    {formatTimestamp(key.last_used)}
                  </td>
                  <td className="py-2 pr-3">
                    {key.is_active ? (
                      <span className="inline-flex items-center gap-1 text-green-400">
                        <span className="w-1.5 h-1.5 rounded-full bg-green-400 inline-block" />
                        Activa
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-surface-500">
                        <span className="w-1.5 h-1.5 rounded-full bg-surface-500 inline-block" />
                        Revocada
                      </span>
                    )}
                  </td>
                  <td className="py-2 text-right">
                    {key.is_active && (
                      confirmingId === key.id ? (
                        <span className="inline-flex items-center gap-1.5">
                          <span className="text-amber-400">¿Confirmar?</span>
                          <button
                            onClick={() => void handleRevoke(key)}
                            disabled={revokingId === key.id}
                            className="text-red-400 hover:text-red-300 font-medium transition-colors"
                          >
                            {revokingId === key.id ? '...' : 'Sí'}
                          </button>
                          <button
                            onClick={() => setConfirmingId(null)}
                            className="text-surface-500 hover:text-surface-300 transition-colors"
                          >
                            No
                          </button>
                        </span>
                      ) : (
                        <button
                          onClick={() => void handleRevoke(key)}
                          className="inline-flex items-center gap-1 text-surface-500 hover:text-red-400 transition-colors"
                          title="Revocar key"
                        >
                          <ShieldOff size={12} />
                          Revocar
                        </button>
                      )
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
