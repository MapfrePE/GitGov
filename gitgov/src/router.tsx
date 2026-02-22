import { createBrowserRouter } from 'react-router-dom'
import { MainLayout } from '@/components/layout/MainLayout'
import { DashboardPage } from '@/pages/DashboardPage'
import { AuditPage } from '@/pages/AuditPage'
import { SettingsPage } from '@/pages/SettingsPage'
import { ControlPlanePage } from '@/pages/ControlPlanePage'

export const router = createBrowserRouter([
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
])
