import { Navigate } from 'react-router-dom'
import { useAuthStore } from '@/store/useAuthStore'
import { Header } from '@/components/layout/Header'
import { AuditLogView } from '@/components/audit/AuditLogView'

export function AuditPage() {
  const { user } = useAuthStore()

  if (!user?.is_admin) {
    return <Navigate to="/" replace />
  }

  return (
    <div className="h-full flex flex-col">
      <Header />
      <AuditLogView />
    </div>
  )
}
