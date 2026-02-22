import { Header } from '@/components/layout/Header'
import { ServerConfigPanel } from '@/components/control_plane/ServerConfigPanel'
import { ServerDashboard } from '@/components/control_plane/ServerDashboard'
import { Button } from '@/components/shared/Button'
import { Link } from 'react-router-dom'

export function ControlPlanePage() {
  return (
    <div className="h-full flex flex-col">
      <Header>
        <Link to="/settings">
          <Button variant="ghost" size="sm">
            Settings
          </Button>
        </Link>
      </Header>
      
      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-6xl mx-auto">
          <div className="grid grid-cols-3 gap-6">
            <div className="col-span-1">
              <ServerConfigPanel />
            </div>
            <div className="col-span-2">
              <ServerDashboard />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
