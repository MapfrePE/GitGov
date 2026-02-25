import { useEffect, useState } from 'react'
import { RouterProvider } from 'react-router-dom'
import { router } from './router'
import { useAuthStore } from './store/useAuthStore'
import { useControlPlaneStore } from './store/useControlPlaneStore'
import { ToastContainer } from './components/shared/Toast'
import { FolderGit2 } from 'lucide-react'

function SplashScreen() {
  return (
    <div className="min-h-[100dvh] bg-surface-950 flex flex-col items-center justify-center">
      <div className="animate-scale-in flex flex-col items-center">
        <div className="w-12 h-12 rounded-xl bg-brand-600 flex items-center justify-center mb-5">
          <FolderGit2 size={24} className="text-white" />
        </div>
        <h1 className="text-xl font-semibold text-white mb-1 tracking-tight">GitGov</h1>
        <p className="text-xs text-surface-500 mb-8">Governance Platform</p>
        <div className="flex gap-1.5">
          <div className="w-1.5 h-1.5 rounded-full bg-surface-500 animate-pulse" />
          <div className="w-1.5 h-1.5 rounded-full bg-surface-600 animate-pulse [animation-delay:150ms]" />
          <div className="w-1.5 h-1.5 rounded-full bg-surface-700 animate-pulse [animation-delay:300ms]" />
        </div>
      </div>
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
