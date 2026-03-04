import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { Button } from '@/components/shared/Button'
import { Spinner } from '@/components/shared/Spinner'
import { Github, ExternalLink, Download, Copy, Check } from 'lucide-react'
import { isTauriDesktop, tauriInvoke } from '@/lib/tauri'

const GITHUB_DEVICE_URL = 'https://github.com/login/device'

export function LoginScreen() {
  const { authStep, deviceFlowInfo, error, startAuth, pollAuth, cancelAuth, clearError } = useAuthStore()
  const [copied, setCopied] = useState(false)
  const isDesktop = isTauriDesktop()

  const handleCopyCode = () => {
    if (deviceFlowInfo) {
      navigator.clipboard.writeText(deviceFlowInfo.user_code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  const handleOpenGitHub = async () => {
    const deviceUrl =
      deviceFlowInfo?.verification_uri?.includes('github.com/login/device')
        ? deviceFlowInfo.verification_uri
        : GITHUB_DEVICE_URL

    if (isDesktop) {
      try {
        await tauriInvoke('cmd_open_external_url', { url: deviceUrl })
        return
      } catch {
        // Fallback to browser open if native command fails.
      }
    }
    window.open(deviceUrl, '_blank', 'noopener,noreferrer')
  }

  if (!isDesktop) {
    return (
      <div className="min-h-dvh bg-surface-950 flex items-center justify-center p-4">
        <div className="max-w-sm w-full animate-fade-in">
          <div className="text-center mb-8">
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-brand-600 mb-5">
              <Github size={24} className="text-white" />
            </div>
            <h1 className="text-2xl font-semibold text-white mb-2 tracking-tight">GitGov</h1>
            <p className="text-sm text-surface-500">Control de flujo Git con roles y auditoría</p>
          </div>

          <div className="glass-card p-6">
            <div className="text-center mb-4">
              <Download size={36} strokeWidth={1.5} className="mx-auto text-surface-400 mb-4" />
              <h2 className="text-lg font-semibold text-white mb-2">
                Requiere GitGov Desktop
              </h2>
              <p className="text-sm text-surface-400">
                La autenticación con GitHub solo está disponible en la aplicación desktop.
              </p>
            </div>

            <div className="bg-surface-900/50 rounded-xl p-4 mb-4 border border-surface-700/30">
              <p className="text-surface-400 text-xs mb-2">
                Para usar todas las funciones de GitGov:
              </p>
              <ol className="text-surface-500 text-xs list-decimal list-inside space-y-1">
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
              <ExternalLink size={16} />
              Descargar GitGov Desktop
            </Button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-dvh bg-surface-950 flex items-center justify-center p-4">
      <div className="relative max-w-sm w-full animate-fade-in">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-brand-600 mb-5">
            <Github size={24} className="text-white" />
          </div>
          <h1 className="text-2xl font-semibold text-white mb-2 tracking-tight">GitGov</h1>
          <p className="text-sm text-surface-500">Control de flujo Git con roles y auditoría</p>
        </div>

        {authStep === 'idle' && (
          <div className="glass-card p-6 animate-slide-up">
            {error && (
              <div className="mb-4 p-3 bg-danger-500/10 border border-danger-500/20 rounded-xl text-danger-400 text-xs flex items-center justify-between">
                <span>{error}</span>
                <button onClick={clearError} className="ml-2 text-danger-400 hover:text-danger-300 underline text-[11px]">
                  Cerrar
                </button>
              </div>
            )}
            <p className="text-sm text-surface-400 text-center mb-5">
              Conecta tu cuenta de GitHub para comenzar
            </p>
            <Button onClick={startAuth} className="w-full" size="lg">
              <Github size={18} />
              Conectar con GitHub
            </Button>
          </div>
        )}

        {authStep === 'waiting_device' && deviceFlowInfo && (
          <div className="glass-card p-6 animate-slide-up">
            <p className="text-sm text-surface-400 text-center mb-5">
              Ve a GitHub, ingresa este código y autoriza GitGov:
            </p>

            <button
              onClick={handleCopyCode}
              className="w-full bg-surface-900/60 rounded-xl p-5 mb-5 text-center border border-surface-700/30 hover:border-surface-600/50 transition-colors group cursor-pointer"
            >
              <code className="text-3xl mono-data text-white tracking-[0.2em] font-semibold">
                {deviceFlowInfo.user_code}
              </code>
              <span className="flex items-center justify-center gap-1.5 mt-3 text-xs text-surface-500 group-hover:text-surface-400 transition-colors">
                {copied ? (
                  <>
                    <Check size={13} className="text-success-400" />
                    <span className="text-success-400">Copiado</span>
                  </>
                ) : (
                  <>
                    <Copy size={13} />
                    <span>Click para copiar</span>
                  </>
                )}
              </span>
            </button>

            <div className="flex gap-3">
              <Button onClick={handleOpenGitHub} variant="secondary" className="flex-1">
                <ExternalLink size={14} />
                Abrir GitHub
              </Button>
              <Button onClick={pollAuth} className="flex-1">
                Continuar
              </Button>
            </div>
            <Button onClick={cancelAuth} variant="ghost" className="w-full mt-3">
              Cancelar
            </Button>
          </div>
        )}

        {authStep === 'polling' && (
          <div className="glass-card p-6 text-center animate-slide-up">
            <Spinner size="lg" className="mx-auto mb-4" />
            <p className="text-white font-medium mb-1 text-sm">Conectando con GitHub...</p>
            <p className="text-surface-500 text-xs">
              Validando autorización del Device Flow
            </p>
            <Button onClick={cancelAuth} variant="ghost" className="w-full mt-4">
              Cancelar
            </Button>
          </div>
        )}
      </div>
    </div>
  )
}
