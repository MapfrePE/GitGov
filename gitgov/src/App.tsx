import { useEffect, useState } from 'react'
import { RouterProvider } from 'react-router-dom'
import { router } from './router'
import { useAuthStore } from './store/useAuthStore'
import { useControlPlaneStore } from './store/useControlPlaneStore'
import { ToastContainer } from './components/shared/Toast'
import { FolderGit2 } from 'lucide-react'

function SplashScreen() {
  return (
    <div className="min-h-screen bg-surface-950 flex flex-col items-center justify-center">
      <div className="animate-scale-in flex flex-col items-center">
        <div className="w-16 h-16 rounded-2xl bg-linear-to-br from-brand-500 to-brand-700 flex items-center justify-center mb-5 shadow-xl shadow-brand-600/20">
          <FolderGit2 size={32} className="text-white" />
        </div>
        <h1 className="text-2xl font-bold text-white mb-1 tracking-tight">GitGov</h1>
        <p className="text-sm text-surface-500 mb-6">Governance Platform</p>
        <div className="flex gap-1">
          <div className="w-2 h-2 rounded-full bg-brand-500 animate-pulse" />
          <div className="w-2 h-2 rounded-full bg-brand-500/60 animate-pulse [animation-delay:150ms]" />
          <div className="w-2 h-2 rounded-full bg-brand-500/30 animate-pulse [animation-delay:300ms]" />
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
