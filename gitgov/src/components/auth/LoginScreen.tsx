import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { Button } from '@/components/shared/Button'
import { Spinner } from '@/components/shared/Spinner'
import { Github, ExternalLink, Download, Copy, Check } from 'lucide-react'
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
      <div className="min-h-screen bg-surface-950 flex items-center justify-center p-4">
        <div className="max-w-md w-full animate-fade-in">
          <div className="text-center mb-8">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-linear-to-br from-brand-500 to-brand-700 mb-5 shadow-xl shadow-brand-600/20">
              <Github size={32} className="text-white" />
            </div>
            <h1 className="text-3xl font-bold text-white mb-2 tracking-tight">GitGov</h1>
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

            <div className="bg-surface-900 rounded-xl p-4 mb-4 border border-surface-700/50">
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
              size="lg"
            >
              <ExternalLink size={18} />
              Descargar GitGov Desktop
            </Button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-surface-950 flex items-center justify-center p-4">
      {/* Subtle gradient background */}
      <div className="fixed inset-0 bg-linear-to-br from-brand-950/30 via-surface-950 to-surface-950 pointer-events-none" />

      <div className="relative max-w-md w-full animate-fade-in">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-linear-to-br from-brand-500 to-brand-700 mb-5 shadow-xl shadow-brand-600/20">
            <Github size={32} className="text-white" />
          </div>
          <h1 className="text-3xl font-bold text-white mb-2 tracking-tight">GitGov</h1>
          <p className="text-surface-400">Control de flujo Git con roles y auditoría</p>
        </div>

        {authStep === 'idle' && (
          <div className="card animate-slide-up">
            {error && (
              <div className="mb-4 p-3 bg-danger-500/10 border border-danger-500/30 rounded-xl text-danger-400 text-sm flex items-center justify-between">
                <span>{error}</span>
                <button onClick={clearError} className="ml-2 text-danger-400 hover:text-danger-300 underline text-xs">
                  Cerrar
                </button>
              </div>
            )}
            <p className="text-surface-300 text-center mb-5">
              Conecta tu cuenta de GitHub para comenzar
            </p>
            <Button onClick={startAuth} className="w-full" size="lg">
              <Github size={20} />
              Conectar con GitHub
            </Button>
          </div>
        )}

        {authStep === 'waiting_device' && deviceFlowInfo && (
          <div className="card animate-slide-up">
            <p className="text-surface-300 text-center mb-5">
              Ve a GitHub, ingresa este código y autoriza GitGov:
            </p>

            <button
              onClick={handleCopyCode}
              className="w-full bg-surface-900 rounded-xl p-5 mb-5 text-center border border-surface-700/50 hover:border-brand-500/30 transition-colors group cursor-pointer"
            >
              <code className="text-3xl font-mono text-brand-400 tracking-[0.2em] font-bold">
                {deviceFlowInfo.user_code}
              </code>
              <span className="flex items-center justify-center gap-1.5 mt-3 text-sm text-surface-400 group-hover:text-surface-300 transition-colors">
                {copied ? (
                  <>
                    <Check size={14} className="text-success-400" />
                    <span className="text-success-400">Copiado</span>
                  </>
                ) : (
                  <>
                    <Copy size={14} />
                    <span>Click para copiar</span>
                  </>
                )}
              </span>
            </button>

            <div className="flex gap-3">
              <Button onClick={handleOpenGitHub} variant="secondary" className="flex-1">
                <ExternalLink size={16} />
                Abrir GitHub
              </Button>
              <Button onClick={pollAuth} className="flex-1">
                Continuar
              </Button>
            </div>
          </div>
        )}

        {authStep === 'polling' && (
          <div className="card text-center animate-slide-up">
            <Spinner size="lg" className="mx-auto mb-4" />
            <p className="text-white font-medium mb-1">Esperando autorización...</p>
            <p className="text-surface-500 text-sm">
              Completa la autorización en GitHub
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
