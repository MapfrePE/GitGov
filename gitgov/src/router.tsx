import { createBrowserRouter, Link, RouterProvider } from 'react-router-dom'
import { MainLayout } from '@/components/layout/MainLayout'
import { DashboardPage } from '@/pages/DashboardPage'
import { AuditPage } from '@/pages/AuditPage'
import { SettingsPage } from '@/pages/SettingsPage'
import { ControlPlanePage } from '@/pages/ControlPlanePage'
import { AlertCircle } from 'lucide-react'

function NotFoundPage() {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center p-8">
      <AlertCircle size={40} className="text-surface-600 mb-4" />
      <h1 className="text-xl font-semibold text-white mb-2">Página no encontrada</h1>
      <p className="text-sm text-surface-400 mb-6">La ruta solicitada no existe.</p>
      <Link
        to="/"
        className="px-4 py-2 bg-brand-600 hover:bg-brand-500 text-white text-sm rounded-lg transition-colors"
      >
        Volver al inicio
      </Link>
    </div>
  )
}

const appRouter = createBrowserRouter([
  {
    path: '/',
    element: (
      <MainLayout>
        <DashboardPage />
      </MainLayout>
    ),
  },
  {
    path: '/audit',
    element: (
      <MainLayout>
        <AuditPage />
      </MainLayout>
    ),
  },
  {
    path: '/settings',
    element: (
      <MainLayout>
        <SettingsPage />
      </MainLayout>
    ),
  },
  {
    path: '/control-plane',
    element: (
      <MainLayout>
        <ControlPlanePage />
      </MainLayout>
    ),
  },
  {
    path: '*',
    element: (
      <MainLayout>
        <NotFoundPage />
      </MainLayout>
    ),
  },
])

export function AppRouter() {
  return <RouterProvider router={appRouter} />
}
