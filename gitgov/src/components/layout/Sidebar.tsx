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
    <div className="w-56 bg-surface-900 border-r border-surface-700/50 flex flex-col">
      {/* Logo */}
      <div className="px-4 py-5 border-b border-surface-700/50">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-linear-to-br from-brand-500 to-brand-700 flex items-center justify-center shadow-lg shadow-brand-600/20">
            <FolderGit2 size={18} className="text-white" />
          </div>
          <div>
            <span className="text-sm font-bold text-white tracking-tight">GitGov</span>
            <span className="block text-[10px] text-surface-500 font-medium">Governance Platform</span>
          </div>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 py-4 space-y-1">
        <p className="px-3 mb-2 text-[10px] font-semibold text-surface-500 uppercase tracking-wider">Navegación</p>
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              clsx(
                'flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all duration-150',
                isActive
                  ? 'bg-brand-600/15 text-brand-400'
                  : 'text-surface-400 hover:bg-surface-800 hover:text-white'
              )
            }
          >
            <item.icon size={18} />
            <span>{item.label}</span>
          </NavLink>
        ))}
      </nav>

      {/* User section */}
      {user && (
        <div className="px-3 py-4 border-t border-surface-700/50">
          <div className="flex items-center gap-3 px-3 mb-3">
            <img
              src={user.avatar_url}
              alt={user.login}
              className="w-8 h-8 rounded-full ring-2 ring-surface-700"
            />
            <div className="flex-1 min-w-0">
              <p className="text-sm text-white font-medium truncate">{user.name || user.login}</p>
              <p className="text-[11px] text-surface-500 truncate">@{user.login}</p>
            </div>
          </div>
          <button
            onClick={logout}
            className="flex items-center gap-2 w-full px-3 py-2 rounded-lg text-sm text-surface-400 hover:bg-surface-800 hover:text-white transition-all duration-150"
          >
            <LogOut size={16} />
            <span>Cerrar sesión</span>
          </button>
        </div>
      )}

      {/* Version */}
      <div className="px-6 py-2 border-t border-surface-700/50">
        <p className="text-[10px] text-surface-600">v1.2-A</p>
      </div>
    </div>
  )
}
