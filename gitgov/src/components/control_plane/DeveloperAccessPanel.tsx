import { useState } from 'react'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Badge } from '@/components/shared/Badge'

export function DeveloperAccessPanel() {
  const { previewOrgInvitation, acceptOrgInvitation } = useControlPlaneStore()
  const [token, setToken] = useState('')
  const [login, setLogin] = useState('')
  const [previewStatus, setPreviewStatus] = useState<string | null>(null)
  const [issuedKey, setIssuedKey] = useState<string | null>(null)

  return (
    <div className="glass-panel p-5 space-y-3">
      <div>
        <div className="card-header">Vista Developer</div>
        <p className="text-xs text-surface-400 mt-1">Puedes aceptar una invitación para habilitar tu API key de acceso.</p>
      </div>

      <input
        value={token}
        onChange={(e) => setToken(e.target.value)}
        placeholder="token de invitación"
        className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
      />
      <input
        value={login}
        onChange={(e) => setLogin(e.target.value)}
        placeholder="login (opcional, si token no lo contiene)"
        className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
      />
      <div className="flex gap-2">
        <button
          type="button"
          className="px-3 py-1.5 rounded bg-white/8 border border-white/15 text-surface-100 text-xs"
          onClick={async () => {
            const preview = await previewOrgInvitation(token.trim())
            if (!preview) {
              setPreviewStatus('Invitación no válida o expirada')
              return
            }
            setPreviewStatus(
              `Estado: ${preview.status} | Rol: ${preview.role} | Expira: ${new Date(preview.expires_at).toLocaleString()}`,
            )
          }}
        >
          Validar token
        </button>
        <button
          type="button"
          className="px-3 py-1.5 rounded bg-brand-500/20 border border-brand-500/40 text-brand-200 text-xs"
          onClick={async () => {
            const accepted = await acceptOrgInvitation({ token: token.trim(), login: login.trim() || undefined })
            if (!accepted) return
            setIssuedKey(accepted.api_key)
            setPreviewStatus(`Acceso habilitado para ${accepted.client_id} (${accepted.role})`)
          }}
        >
          Aceptar invitación
        </button>
      </div>

      {previewStatus && <Badge variant="neutral">{previewStatus}</Badge>}
      {issuedKey && (
        <div className="space-y-1">
          <div className="text-[11px] text-surface-300">API key emitida:</div>
          <code className="block text-[10px] text-brand-200 break-all">{issuedKey}</code>
        </div>
      )}
    </div>
  )
}
