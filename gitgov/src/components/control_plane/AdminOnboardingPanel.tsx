import { useEffect, useMemo, useState } from 'react'
import { Badge } from '@/components/shared/Badge'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { formatTs } from '@/lib/timezone'

export function AdminOnboardingPanel() {
  const selectedOrgName = useControlPlaneStore((s) => s.selectedOrgName)
  const setSelectedOrgName = useControlPlaneStore((s) => s.setSelectedOrgName)
  const createOrg = useControlPlaneStore((s) => s.createOrg)
  const orgUsers = useControlPlaneStore((s) => s.orgUsers)
  const orgInvitations = useControlPlaneStore((s) => s.orgInvitations)
  const lastGeneratedInviteToken = useControlPlaneStore((s) => s.lastGeneratedInviteToken)
  const loadOrgUsers = useControlPlaneStore((s) => s.loadOrgUsers)
  const loadOrgInvitations = useControlPlaneStore((s) => s.loadOrgInvitations)
  const createOrgInvitation = useControlPlaneStore((s) => s.createOrgInvitation)
  const resendOrgInvitation = useControlPlaneStore((s) => s.resendOrgInvitation)
  const revokeOrgInvitation = useControlPlaneStore((s) => s.revokeOrgInvitation)
  const issueApiKeyForOrgUser = useControlPlaneStore((s) => s.issueApiKeyForOrgUser)
  const updateOrgUserStatus = useControlPlaneStore((s) => s.updateOrgUserStatus)
  const upsertOrgUser = useControlPlaneStore((s) => s.upsertOrgUser)
  const displayTimezone = useControlPlaneStore((s) => s.displayTimezone)

  const [orgLogin, setOrgLogin] = useState(selectedOrgName)
  const [orgName, setOrgName] = useState('')
  const [memberLogin, setMemberLogin] = useState('')
  const [memberEmail, setMemberEmail] = useState('')
  const [memberRole, setMemberRole] = useState('Developer')
  const [inviteLogin, setInviteLogin] = useState('')
  const [inviteEmail, setInviteEmail] = useState('')
  const [inviteRole, setInviteRole] = useState('Developer')
  const [issuedKeys, setIssuedKeys] = useState<Record<string, string>>({})

  useEffect(() => {
    const orgName = selectedOrgName.trim() || undefined
    void Promise.all([
      loadOrgUsers({ orgName, limit: 100 }),
      loadOrgInvitations({ orgName, limit: 100 }),
    ])
  }, [selectedOrgName, loadOrgUsers, loadOrgInvitations])

  const hasOrg = selectedOrgName.trim().length > 0
  const activeMembers = useMemo(() => orgUsers.filter((u) => u.status === 'active'), [orgUsers])

  return (
    <div className="glass-panel p-5 space-y-4">
      <div>
        <div className="card-header">Onboarding Admin</div>
        <p className="text-xs text-surface-400 mt-1">Crea organización, invita developers y administra acceso por rol.</p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
        <div className="rounded-lg border border-white/8 p-3 bg-white/2 space-y-2">
          <div className="text-[11px] text-surface-300 uppercase tracking-widest">1. Crear organización</div>
          <input
            value={orgLogin}
            onChange={(e) => setOrgLogin(e.target.value)}
            placeholder="login org (ej: acme)"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <input
            value={orgName}
            onChange={(e) => setOrgName(e.target.value)}
            placeholder="nombre visible (opcional)"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <div className="flex gap-2">
            <button
              type="button"
              className="px-3 py-1.5 rounded bg-brand-500/20 border border-brand-500/40 text-brand-200 text-xs"
              onClick={async () => {
                const created = await createOrg({ login: orgLogin, name: orgName || undefined })
                if (created?.login) {
                  setSelectedOrgName(created.login)
                }
              }}
            >
              Crear/Upsert Org
            </button>
            {hasOrg && <Badge variant="success">Org activa: {selectedOrgName}</Badge>}
          </div>
        </div>

        <div className="rounded-lg border border-white/8 p-3 bg-white/2 space-y-2">
          <div className="text-[11px] text-surface-300 uppercase tracking-widest">2. Definir org activa</div>
          <input
            value={selectedOrgName}
            onChange={(e) => setSelectedOrgName(e.target.value)}
            placeholder="org_name para scope admin"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <div className="flex items-center gap-2 text-[11px] text-surface-400">
            <span>Miembros activos:</span>
            <span className="mono-data text-surface-200">{activeMembers.length}</span>
            <span className="text-surface-600">Invitaciones:</span>
            <span className="mono-data text-surface-200">{orgInvitations.length}</span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
        <div className="rounded-lg border border-white/8 p-3 bg-white/2 space-y-2">
          <div className="text-[11px] text-surface-300 uppercase tracking-widest">3A. Provisionar miembro directo</div>
          <input
            value={memberLogin}
            onChange={(e) => setMemberLogin(e.target.value)}
            placeholder="login (requerido)"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <input
            value={memberEmail}
            onChange={(e) => setMemberEmail(e.target.value)}
            placeholder="email (opcional)"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <select
            value={memberRole}
            onChange={(e) => setMemberRole(e.target.value)}
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          >
            <option>Admin</option>
            <option>Architect</option>
            <option>Developer</option>
            <option>PM</option>
          </select>
          <button
            type="button"
            className="px-3 py-1.5 rounded bg-white/8 border border-white/15 text-surface-100 text-xs"
            onClick={async () => {
              if (!memberLogin.trim()) return
              await upsertOrgUser({
                orgName: selectedOrgName,
                login: memberLogin,
                email: memberEmail || undefined,
                role: memberRole,
                status: 'active',
              })
              setMemberLogin('')
              setMemberEmail('')
            }}
          >
            Guardar miembro
          </button>
        </div>

        <div className="rounded-lg border border-white/8 p-3 bg-white/2 space-y-2">
          <div className="text-[11px] text-surface-300 uppercase tracking-widest">3B. Invitar developer</div>
          <input
            value={inviteLogin}
            onChange={(e) => setInviteLogin(e.target.value)}
            placeholder="login destino (opcional)"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <input
            value={inviteEmail}
            onChange={(e) => setInviteEmail(e.target.value)}
            placeholder="email destino (opcional)"
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          />
          <select
            value={inviteRole}
            onChange={(e) => setInviteRole(e.target.value)}
            className="w-full bg-surface-900 border border-white/10 rounded px-2 py-1.5 text-xs text-surface-100"
          >
            <option>Developer</option>
            <option>Admin</option>
            <option>Architect</option>
            <option>PM</option>
          </select>
          <button
            type="button"
            className="px-3 py-1.5 rounded bg-brand-500/20 border border-brand-500/40 text-brand-200 text-xs"
            onClick={async () => {
              const result = await createOrgInvitation({
                orgName: selectedOrgName,
                inviteLogin: inviteLogin || undefined,
                inviteEmail: inviteEmail || undefined,
                role: inviteRole,
                expiresInDays: 7,
              })
              if (result) {
                setInviteLogin('')
                setInviteEmail('')
              }
            }}
          >
            Generar invitación
          </button>
          {lastGeneratedInviteToken && (
            <div className="text-[11px] text-surface-300 space-y-1">
              <div>Token generado:</div>
              <div className="flex gap-2 items-center">
                <code className="text-[10px] break-all text-brand-200">{lastGeneratedInviteToken}</code>
                <button
                  type="button"
                  className="text-[10px] text-surface-400 hover:text-surface-200"
                  onClick={() => void navigator.clipboard.writeText(lastGeneratedInviteToken)}
                >
                  copiar
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
        <div className="rounded-lg border border-white/8 p-3 bg-white/2">
          <div className="text-[11px] text-surface-300 uppercase tracking-widest mb-2">Miembros por rol</div>
          <div className="max-h-64 overflow-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="text-surface-500 text-[10px]">
                  <th className="text-left py-1">Login</th>
                  <th className="text-left py-1">Rol</th>
                  <th className="text-left py-1">Estado</th>
                  <th className="text-left py-1">API Key</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {orgUsers.map((user) => (
                  <tr key={user.id}>
                    <td className="py-1.5 text-surface-200">{user.login}</td>
                    <td className="py-1.5 text-surface-300">{user.role}</td>
                    <td className="py-1.5">
                      <button
                        type="button"
                        className="text-[10px] text-surface-300 hover:text-surface-100"
                        onClick={() => void updateOrgUserStatus(user.id, user.status === 'active' ? 'disabled' : 'active')}
                      >
                        {user.status}
                      </button>
                    </td>
                    <td className="py-1.5">
                      <button
                        type="button"
                        className="text-[10px] text-brand-300 hover:text-brand-200"
                        onClick={async () => {
                          const key = await issueApiKeyForOrgUser(user.id)
                          if (key?.api_key) {
                            setIssuedKeys((prev) => ({ ...prev, [user.id]: key.api_key as string }))
                          }
                        }}
                      >
                        emitir
                      </button>
                      {issuedKeys[user.id] && (
                        <div className="mt-1 text-[10px] text-brand-200 break-all">{issuedKeys[user.id]}</div>
                      )}
                    </td>
                  </tr>
                ))}
                {orgUsers.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-6 text-center text-surface-600">Sin miembros.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>

        <div className="rounded-lg border border-white/8 p-3 bg-white/2">
          <div className="text-[11px] text-surface-300 uppercase tracking-widest mb-2">Invitaciones</div>
          <div className="max-h-64 overflow-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="text-surface-500 text-[10px]">
                  <th className="text-left py-1">Destino</th>
                  <th className="text-left py-1">Rol</th>
                  <th className="text-left py-1">Estado</th>
                  <th className="text-left py-1">Acción</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {orgInvitations.map((invitation) => (
                  <tr key={invitation.id}>
                    <td className="py-1.5 text-surface-200">{invitation.invite_login || invitation.invite_email || '-'}</td>
                    <td className="py-1.5 text-surface-300">{invitation.role}</td>
                    <td className="py-1.5 text-surface-300">{invitation.status}<div className="text-[10px] text-surface-600">{formatTs(invitation.expires_at, displayTimezone)}</div></td>
                    <td className="py-1.5 space-x-2">
                      <button
                        type="button"
                        className="text-[10px] text-brand-300 hover:text-brand-200"
                        onClick={() => void resendOrgInvitation(invitation.id, 7)}
                      >
                        reenviar
                      </button>
                      <button
                        type="button"
                        className="text-[10px] text-rose-300 hover:text-rose-200"
                        onClick={() => void revokeOrgInvitation(invitation.id)}
                      >
                        revocar
                      </button>
                    </td>
                  </tr>
                ))}
                {orgInvitations.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-6 text-center text-surface-600">Sin invitaciones.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  )
}
