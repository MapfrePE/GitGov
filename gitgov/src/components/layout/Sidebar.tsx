import { NavLink } from 'react-router-dom'
import { useAuthStore } from '@/store/useAuthStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { GitBranch, Settings, LogOut, Shield, Server, HelpCircle } from 'lucide-react'
import clsx from 'clsx'

export function Sidebar() {
  const { user, logout } = useAuthStore()
  const disconnect = useControlPlaneStore((s) => s.disconnect)

  const navItems = [
    { to: '/', icon: GitBranch, label: 'Inicio' },
    { to: '/control-plane', icon: Server, label: 'Control Plane' },
    ...(user?.is_admin ? [{ to: '/audit', icon: Shield, label: 'Auditoría' }] : []),
    { to: '/settings', icon: Settings, label: 'Configuración' },
    { to: '/help', icon: HelpCircle, label: 'Ayuda / FAQ' },
  ]

  return (
    <div className="w-18 bg-surface-950 border-r border-white/4 flex flex-col items-center py-4">
      {/* Logo */}
      <div className="mb-7">
        <img src="/logo.png" alt="GitGov" className="w-12 h-12 object-contain" />
      </div>

      {/* Navigation */}
      <nav className="flex-1 flex flex-col items-center gap-1">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            title={item.label}
            aria-label={item.label}
            className={({ isActive }) =>
              clsx(
                'w-10 h-10 flex items-center justify-center rounded-lg transition-all duration-200',
                isActive
                  ? 'bg-white/[0.08] text-white'
                  : 'text-surface-500 hover:text-surface-300 hover:bg-white/4'
              )
            }
          >
            <item.icon size={18} strokeWidth={1.5} aria-hidden="true" />
          </NavLink>
        ))}
      </nav>

      {/* User + Logout */}
      {user && (
        <div className="flex flex-col items-center gap-2 mt-auto">
          <img
            src={user.avatar_url}
            alt={user.login}
            title={user.login}
            className="w-8 h-8 rounded-full opacity-70 hover:opacity-100 transition-opacity"
          />
          <button
            onClick={async () => {
              disconnect()
              await logout()
            }}
            title="Cambiar usuario (cerrar sesión)"
            aria-label="Cambiar usuario (cerrar sesión)"
            className="w-10 h-10 flex items-center justify-center rounded-lg text-surface-600 hover:text-surface-400 hover:bg-white/4 transition-all duration-200"
          >
            <LogOut size={16} strokeWidth={1.5} aria-hidden="true" />
          </button>
        </div>
      )}
    </div>
  )
}
