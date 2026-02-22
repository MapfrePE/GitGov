import { useEffect, useState } from 'react'
import { RouterProvider } from 'react-router-dom'
import { router } from './router'
import { useAuthStore } from './store/useAuthStore'
import { useControlPlaneStore } from './store/useControlPlaneStore'
import { ToastContainer } from './components/shared/Toast'
import { Spinner } from './components/shared/Spinner'
import { FolderGit2 } from 'lucide-react'

function SplashScreen() {
  return (
    <div className="min-h-screen bg-surface-900 flex flex-col items-center justify-center">
      <div className="w-16 h-16 rounded-2xl bg-brand-600 flex items-center justify-center mb-4">
        <FolderGit2 size={32} />
      </div>
      <h1 className="text-2xl font-bold text-white mb-4">GitGov</h1>
      <Spinner size="lg" />
    </div>
  )
}

function App() {
  const { checkExistingSession, isLoading } = useAuthStore()
  const { initFromEnv } = useControlPlaneStore()
  const [initialized, setInitialized] = useState(false)

  useEffect(() => {
    const init = async () => {
      await Promise.all([
        checkExistingSession(),
        initFromEnv(),
      ])
      setInitialized(true)
    }
    init()
  }, [checkExistingSession, initFromEnv])

  if (!initialized || isLoading) {
    return <SplashScreen />
  }

  return (
    <>
      <RouterProvider router={router} />
      <ToastContainer />
    </>
  )
}

export default App
