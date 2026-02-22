import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { Button } from '@/components/shared/Button'
import { Spinner } from '@/components/shared/Spinner'
import { Github, ExternalLink, Download } from 'lucide-react'
import { isTauriDesktop } from '@/lib/tauri'

export function LoginScreen() {
  const { authStep, deviceFlowInfo, error, startAuth, pollAuth, clearError } = useAuthStore()
  const [copied, setCopied] = useState(false)
  const isDesktop = isTauriDesktop()

  const handleCopyCode = () => {
    if (deviceFlowInfo) {
      navigator.clipboard.writeText(deviceFlowInfo.user_code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  const handleOpenGitHub = () => {
    if (deviceFlowInfo) {
      window.open(deviceFlowInfo.verification_uri, '_blank')
    }
  }

  // Not running in Tauri desktop - show message
  if (!isDesktop) {
    return (
      <div className="min-h-screen bg-surface-900 flex items-center justify-center p-4">
        <div className="max-w-md w-full">
          <div className="text-center mb-8">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-brand-600 mb-4">
              <Github size={32} />
            </div>
            <h1 className="text-3xl font-bold text-white mb-2">GitGov</h1>
            <p className="text-surface-400">Control de flujo Git con roles y auditoría</p>
          </div>

          <div className="card">
            <div className="text-center mb-4">
              <Download size={48} className="mx-auto text-brand-400 mb-4" />
              <h2 className="text-xl font-semibold text-white mb-2">
                Requiere GitGov Desktop
              </h2>
              <p className="text-surface-300">
                La autenticación con GitHub solo está disponible en la aplicación desktop.
              </p>
            </div>
            
            <div className="bg-surface-700 rounded-lg p-4 mb-4">
              <p className="text-surface-300 text-sm">
                Para usar todas las funciones de GitGov:
              </p>
              <ol className="text-surface-400 text-sm list-decimal list-inside mt-2 space-y-1">
                <li>Descarga GitGov Desktop</li>
                <li>Instala la aplicación</li>
                <li>Abre la aplicación para autenticarte</li>
              </ol>
            </div>

            <Button 
              onClick={() => window.open('https://github.com/MapfrePE/GitGov', '_blank')} 
              className="w-full"
            >
              <ExternalLink size={20} className="mr-2" />
              Descargar GitGov Desktop
            </Button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-surface-900 flex items-center justify-center p-4">
      <div className="max-w-md w-full">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-brand-600 mb-4">
            <Github size={32} />
          </div>
          <h1 className="text-3xl font-bold text-white mb-2">GitGov</h1>
          <p className="text-surface-400">Control de flujo Git con roles y auditoría</p>
        </div>

        {authStep === 'idle' && (
          <div className="card">
            {error && (
              <div className="mb-4 p-3 bg-danger-500/20 border border-danger-500/50 rounded-lg text-danger-400 text-sm">
                {error}
                <button onClick={clearError} className="ml-2 underline">
                  Cerrar
                </button>
              </div>
            )}
            <p className="text-surface-300 text-center mb-4">
              Conecta tu cuenta de GitHub para comenzar
            </p>
            <Button onClick={startAuth} className="w-full" size="lg">
              <Github size={20} className="mr-2" />
              Conectar con GitHub
            </Button>
          </div>
        )}

        {authStep === 'waiting_device' && deviceFlowInfo && (
          <div className="card">
            <p className="text-surface-300 text-center mb-4">
              Ve a GitHub, ingresa este código y autoriza GitGov:
            </p>
            
            <div className="bg-surface-700 rounded-lg p-4 mb-4 text-center">
              <code className="text-2xl font-mono text-brand-400 tracking-wider">
                {deviceFlowInfo.user_code}
              </code>
              <button
                onClick={handleCopyCode}
                className="block mt-2 text-sm text-surface-400 hover:text-white"
              >
                {copied ? 'Copiado!' : 'Copiar código'}
              </button>
            </div>

            <div className="flex gap-2">
              <Button onClick={handleOpenGitHub} variant="secondary" className="flex-1">
                <ExternalLink size={16} className="mr-2" />
                Abrir GitHub
              </Button>
              <Button onClick={pollAuth} className="flex-1">
                Continuar
              </Button>
            </div>
          </div>
        )}

        {authStep === 'polling' && (
          <div className="card text-center">
            <Spinner size="lg" className="mx-auto mb-4" />
            <p className="text-surface-300">Esperando autorización...</p>
            <p className="text-surface-500 text-sm mt-2">
              Completa la autorización en GitHub
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
