import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'
import { Lock } from 'lucide-react'

export function PinUnlockScreen() {
  const { user, pinError, unlockWithPin, logout } = useAuthStore()
  const disconnect = useControlPlaneStore((s) => s.disconnect)
  const [pin, setPin] = useState('')

  const handleUnlock = () => {
    unlockWithPin(pin)
    setPin('')
  }

  return (
    <div className="min-h-dvh bg-surface-950 flex items-center justify-center p-4">
      <div className="max-w-sm w-full animate-fade-in">
        <div className="glass-card p-6">
          <div className="text-center mb-5">
            <div className="inline-flex items-center justify-center w-11 h-11 rounded-xl bg-brand-600 mb-4">
              <Lock size={20} className="text-white" />
            </div>
            <h2 className="text-lg font-semibold text-white">Desbloquear sesión</h2>
            <p className="text-xs text-surface-500 mt-1">
              Ingresa tu PIN local para continuar{user ? ` (@${user.login})` : ''}.
            </p>
          </div>

          <input
            type="password"
            inputMode="numeric"
            pattern="[0-9]*"
            placeholder="PIN (4 a 6 dígitos)"
            value={pin}
            onChange={(e) => setPin(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleUnlock()
            }}
            className="w-full bg-surface-900/60 rounded-xl border border-surface-700/30 px-3 py-2 text-sm text-white mb-3 outline-none focus:border-brand-500/60"
          />

          {pinError && (
            <p className="text-[11px] text-danger-400 mb-3">{pinError}</p>
          )}

          <div className="flex gap-2">
            <Button className="flex-1" onClick={handleUnlock}>
              Desbloquear
            </Button>
            <Button
              variant="secondary"
              onClick={async () => {
                disconnect()
                await logout()
              }}
            >
              Cambiar usuario
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
