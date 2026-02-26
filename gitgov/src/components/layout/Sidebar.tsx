import { NavLink } from 'react-router-dom'
import { useAuthStore } from '@/store/useAuthStore'
import { GitBranch, Settings, LogOut, FolderGit2, Shield, Server } from 'lucide-react'
import clsx from 'clsx'

export function Sidebar() {
  const { user, logout } = useAuthStore()

  const navItems = [
    { to: '/', icon: GitBranch, label: 'Inicio' },
    { to: '/control-plane', icon: Server, label: 'Control Plane' },
    ...(user?.is_admin ? [{ to: '/audit', icon: Shield, label: 'Auditoría' }] : []),
    { to: '/settings', icon: Settings, label: 'Configuración' },
  ]

  return (
    <div className="w-14 bg-surface-950 border-r border-white/[0.04] flex flex-col items-center py-3">
      {/* Logo */}
      <div className="w-8 h-8 rounded-lg bg-brand-600 flex items-center justify-center mb-6">
        <FolderGit2 size={15} className="text-white" />
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
                'w-9 h-9 flex items-center justify-center rounded-lg transition-all duration-200',
                isActive
                  ? 'bg-white/[0.08] text-white'
                  : 'text-surface-500 hover:text-surface-300 hover:bg-white/[0.04]'
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
            className="w-7 h-7 rounded-full opacity-70 hover:opacity-100 transition-opacity"
          />
          <button
            onClick={logout}
            title="Cerrar sesión"
            aria-label="Cerrar sesión"
            className="w-9 h-9 flex items-center justify-center rounded-lg text-surface-600 hover:text-surface-400 hover:bg-white/[0.04] transition-all duration-200"
          >
            <LogOut size={16} strokeWidth={1.5} aria-hidden="true" />
          </button>
        </div>
      )}
    </div>
  )
}
