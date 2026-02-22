import { NavLink } from 'react-router-dom'
import { useAuthStore } from '@/store/useAuthStore'
import { GitBranch, Settings, LogOut, FolderGit2, Shield, Server } from 'lucide-react'
import clsx from 'clsx'

export function Sidebar() {
  const { user, logout } = useAuthStore()

  const navItems = [
    { to: '/', icon: GitBranch, label: 'Dashboard' },
    { to: '/control-plane', icon: Server, label: 'Control Plane' },
    ...(user?.is_admin ? [{ to: '/audit', icon: Shield, label: 'Auditoría' }] : []),
    { to: '/settings', icon: Settings, label: 'Configuración' },
  ]

  return (
    <div className="w-16 bg-surface-800 border-r border-surface-700 flex flex-col items-center py-4">
      <div className="mb-8">
        <div className="w-10 h-10 rounded-xl bg-brand-600 flex items-center justify-center">
          <FolderGit2 size={20} />
        </div>
      </div>

      <nav className="flex-1 flex flex-col gap-2">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              clsx(
                'w-10 h-10 rounded-lg flex items-center justify-center transition-colors',
                isActive
                  ? 'bg-brand-600 text-white'
                  : 'text-surface-400 hover:bg-surface-700 hover:text-white'
              )
            }
            title={item.label}
          >
            <item.icon size={20} />
          </NavLink>
        ))}
      </nav>

      {user && (
        <div className="flex flex-col items-center gap-2">
          <img
            src={user.avatar_url}
            alt={user.login}
            className="w-8 h-8 rounded-full"
          />
          <button
            onClick={logout}
            className="w-10 h-10 rounded-lg flex items-center justify-center text-surface-400 hover:bg-surface-700 hover:text-white transition-colors"
            title="Cerrar sesión"
          >
            <LogOut size={20} />
          </button>
        </div>
      )}
    </div>
  )
}
