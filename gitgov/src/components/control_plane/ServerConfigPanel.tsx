import { useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Server, Link, Unlink, RefreshCw } from 'lucide-react'

export function ServerConfigPanel() {
  const { serverConfig, isConnected, isLoading, error, setServerConfig, checkConnection, disconnect } = useControlPlaneStore()
  const [url, setUrl] = useState(serverConfig?.url || 'http://localhost:3000')
  const [apiKey, setApiKey] = useState(serverConfig?.api_key || '')

  const handleConnect = () => {
    setServerConfig({
      url,
      api_key: apiKey || undefined,
    })
  }

  if (isConnected && serverConfig) {
    return (
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Server size={20} className="text-success-500" />
            <span className="text-white font-medium">Connected to Control Plane</span>
          </div>
          <div className="flex gap-2">
            <Button variant="ghost" size="sm" onClick={checkConnection}>
              <RefreshCw size={14} className={isLoading ? 'animate-spin' : ''} />
            </Button>
            <Button variant="danger" size="sm" onClick={disconnect}>
              <Unlink size={14} className="mr-1" />
              Disconnect
            </Button>
          </div>
        </div>
        
        <div className="bg-surface-900 rounded-lg p-3">
          <p className="text-xs text-surface-400 mb-1">Server URL</p>
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
        <span className="text-white font-medium">Connect to Control Plane</span>
      </div>
      
      <div className="space-y-3">
        <div>
          <label className="block text-sm text-surface-400 mb-1">Server URL</label>
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="http://localhost:3000"
            className="input"
          />
        </div>
        
        <div>
          <label className="block text-sm text-surface-400 mb-1">API Key (optional)</label>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="Your API key"
            className="input"
          />
        </div>
        
        <Button onClick={handleConnect} loading={isLoading} className="w-full">
          <Link size={16} className="mr-2" />
          Connect
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
