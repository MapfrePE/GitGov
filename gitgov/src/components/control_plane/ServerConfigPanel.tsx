import { useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Server, Link, Unlink, RefreshCw, Wrench } from 'lucide-react'

const DEV_LOCAL_SERVER_URL = 'http://127.0.0.1:3000'
const IS_DEV_MODE = Boolean(import.meta.env.DEV)

export function ServerConfigPanel() {
  const serverConfig = useControlPlaneStore((s) => s.serverConfig)
  const isConnected = useControlPlaneStore((s) => s.isConnected)
  const connectionStatus = useControlPlaneStore((s) => s.connectionStatus)
  const isLoading = useControlPlaneStore((s) => s.isLoading)
  const error = useControlPlaneStore((s) => s.error)
  const userRole = useControlPlaneStore((s) => s.userRole)
  const userClientId = useControlPlaneStore((s) => s.userClientId)
  const setServerConfig = useControlPlaneStore((s) => s.setServerConfig)
  const checkConnection = useControlPlaneStore((s) => s.checkConnection)
  const disconnect = useControlPlaneStore((s) => s.disconnect)
  const [url, setUrl] = useState(IS_DEV_MODE ? DEV_LOCAL_SERVER_URL : (serverConfig?.url || DEV_LOCAL_SERVER_URL))
  const [apiKey, setApiKey] = useState(serverConfig?.api_key || '')

  const handleConnect = () => {
    setServerConfig({
      url: IS_DEV_MODE ? DEV_LOCAL_SERVER_URL : url,
      api_key: apiKey || undefined,
    })
  }

  if (connectionStatus === 'maintenance' && serverConfig) {
    return (
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Wrench size={20} className="text-warning-400" />
            <span className="text-white font-medium">Servidor en mantenimiento</span>
          </div>
          <div className="flex gap-2">
            <Button variant="ghost" size="sm" onClick={() => void checkConnection()}>
              <RefreshCw size={14} className={isLoading ? 'animate-spin' : ''} />
            </Button>
          </div>
        </div>

        <div className="bg-surface-900 rounded-lg p-3">
          <p className="text-xs text-surface-400 mb-1">URL del servidor</p>
          <p className="text-sm text-white font-mono">{serverConfig.url}</p>
        </div>

        <div className="bg-surface-900 rounded-lg p-3 mt-3">
          <p className="text-xs text-surface-400 mb-1">Identidad Control Plane</p>
          <p className="text-sm text-white">
            {userRole || 'sin rol'}{userClientId ? ` · ${userClientId}` : ''}
          </p>
        </div>

        {userRole !== 'Admin' && (
          <div className="mt-3 p-2 bg-warning-500/20 border border-warning-500/50 rounded text-warning-300 text-sm">
            Estás autenticado como {userRole || 'sin rol'}. Para founder/admin usa una API key Admin.
          </div>
        )}

        <div className="mt-3 p-2 bg-warning-500/20 border border-warning-500/50 rounded text-warning-300 text-sm flex items-center gap-2">
          <Wrench size={14} className="shrink-0" />
          El servidor se está actualizando. Reconectando cada 10 segundos...
        </div>
      </div>
    )
  }

  if (isConnected && serverConfig) {
    return (
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Server size={20} className="text-success-500" />
            <span className="text-white font-medium">Conectado al Control Plane</span>
          </div>
          <div className="flex gap-2">
            <Button variant="ghost" size="sm" onClick={() => void checkConnection()}>
              <RefreshCw size={14} className={isLoading ? 'animate-spin' : ''} />
            </Button>
            <Button variant="danger" size="sm" onClick={disconnect}>
              <Unlink size={14} className="mr-1" />
              Desconectar
            </Button>
          </div>
        </div>
        
        <div className="bg-surface-900 rounded-lg p-3">
          <p className="text-xs text-surface-400 mb-1">URL del servidor</p>
          <p className="text-sm text-white font-mono">{serverConfig.url}</p>
        </div>
        
        {error && (
          <div className="mt-3 p-2 bg-danger-500/20 border border-danger-500/50 rounded text-danger-400 text-sm">
            {error}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="card">
      <div className="flex items-center gap-2 mb-4">
        <Link size={20} className="text-brand-500" />
        <span className="text-white font-medium">Conectar al Control Plane</span>
      </div>
      
      <div className="space-y-3">
        <div>
          <label htmlFor="server-url-input" className="block text-sm text-surface-400 mb-1">URL del servidor</label>
          <input
            id="server-url-input"
            type="text"
            value={IS_DEV_MODE ? DEV_LOCAL_SERVER_URL : url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="http://127.0.0.1:3000"
            disabled={IS_DEV_MODE}
            className="input"
          />
          {IS_DEV_MODE && (
            <p className="mt-1 text-xs text-warning-400">
              Modo desarrollo: la URL está fijada a {DEV_LOCAL_SERVER_URL} para evitar apuntar a servidores remotos.
            </p>
          )}
        </div>
        
        <div>
          <label htmlFor="server-api-key-input" className="block text-sm text-surface-400 mb-1">API Key (opcional)</label>
          <input
            id="server-api-key-input"
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="Tu API key"
            className="input"
          />
        </div>
        
        <Button onClick={handleConnect} loading={isLoading} className="w-full">
          <Link size={16} className="mr-2" />
          Conectar
        </Button>
        {error && (
          <div className="p-2 bg-danger-500/20 border border-danger-500/50 rounded text-danger-400 text-sm">
            {error}
          </div>
        )}
      </div>
    </div>
  )
}
