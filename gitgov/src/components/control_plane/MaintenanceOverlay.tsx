import { useState, useEffect } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Wrench, RefreshCw, ShieldCheck } from 'lucide-react'
import { Button } from '@/components/shared/Button'

export function MaintenanceOverlay() {
  const maintenanceDetectedAt = useControlPlaneStore((s) => s.maintenanceDetectedAt)
  const checkConnection = useControlPlaneStore((s) => s.checkConnection)
  const isLoading = useControlPlaneStore((s) => s.isLoading)
  const serverStats = useControlPlaneStore((s) => s.serverStats)
  const [elapsed, setElapsed] = useState(0)

  // Update elapsed time every second
  useEffect(() => {
    if (!maintenanceDetectedAt) return
    const tick = () => setElapsed(Math.floor((Date.now() - maintenanceDetectedAt) / 1000))
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [maintenanceDetectedAt])

  // Auto-retry every 10 seconds during maintenance
  useEffect(() => {
    const id = setInterval(() => {
      void checkConnection()
    }, 10_000)
    return () => clearInterval(id)
  }, [checkConnection])

  const minutes = Math.floor(elapsed / 60)
  const seconds = elapsed % 60
  const elapsedStr = minutes > 0
    ? `${minutes}m ${seconds.toString().padStart(2, '0')}s`
    : `${seconds}s`

  return (
    <div className="flex flex-col items-center justify-center min-h-[420px] animate-fade-in px-4">
      {/* Animated wrench icon */}
      <div className="relative mb-6">
        <div className="w-16 h-16 rounded-2xl bg-warning-500/10 border border-warning-500/20 flex items-center justify-center">
          <Wrench
            size={28}
            strokeWidth={1.5}
            className="text-warning-400 animate-[spin_4s_ease-in-out_infinite]"
          />
        </div>
        <span className="absolute -bottom-1 -right-1 w-4 h-4 rounded-full bg-warning-500 animate-pulse" />
      </div>

      {/* Title */}
      <h2 className="text-base font-semibold text-surface-100 mb-1">
        Servidor en mantenimiento
      </h2>
      <p className="text-xs text-surface-400 text-center max-w-xs mb-6">
        El sistema se está actualizando. Reconectando automáticamente...
      </p>

      {/* Progress bar animation */}
      <div className="w-48 h-1 rounded-full bg-surface-800 overflow-hidden mb-5">
        <div className="h-full w-1/3 rounded-full bg-warning-500/70 animate-[shimmer_1.5s_ease-in-out_infinite]" />
      </div>

      {/* Elapsed counter */}
      <div className="flex items-center gap-4 mb-6">
        <div className="text-center">
          <p className="text-[10px] text-surface-600 uppercase tracking-wider mb-0.5">Tiempo de inactividad</p>
          <p className="text-sm font-mono text-warning-400 font-medium">{elapsedStr}</p>
        </div>
        {serverStats && (
          <div className="text-center border-l border-surface-700/50 pl-4">
            <p className="text-[10px] text-surface-600 uppercase tracking-wider mb-0.5">Último estado</p>
            <p className="text-sm font-mono text-surface-300 font-medium">
              {serverStats.client_events.total.toLocaleString()} eventos
            </p>
          </div>
        )}
      </div>

      {/* Reassurance */}
      <div className="flex items-center gap-2 text-[11px] text-surface-500 bg-surface-800/60 border border-surface-700/30 rounded-lg px-4 py-2.5 mb-5">
        <ShieldCheck size={14} strokeWidth={1.5} className="text-success-500 shrink-0" />
        <span>Tus eventos locales están seguros en la cola de salida</span>
      </div>

      {/* Manual retry button */}
      <Button
        variant="ghost"
        size="sm"
        onClick={() => void checkConnection()}
        loading={isLoading}
      >
        <RefreshCw size={13} strokeWidth={1.5} className={isLoading ? 'animate-spin' : ''} />
        Reintentar ahora
      </Button>
    </div>
  )
}
