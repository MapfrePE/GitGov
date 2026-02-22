import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { AuthenticatedUser, DeviceFlowInfo } from '@/lib/types'

type AuthStep = 'idle' | 'waiting_device' | 'polling' | 'authenticated'

interface AuthState {
  user: AuthenticatedUser | null
  isLoading: boolean
  authStep: AuthStep
  deviceFlowInfo: DeviceFlowInfo | null
  error: string | null
}

interface AuthActions {
  startAuth: () => Promise<void>
  pollAuth: () => Promise<void>
  checkExistingSession: () => Promise<void>
  logout: () => Promise<void>
  setUser: (user: AuthenticatedUser) => void
  clearError: () => void
}

export const useAuthStore = create<AuthState & AuthActions>((set, get) => ({
  user: null,
  isLoading: false,
  authStep: 'idle',
  deviceFlowInfo: null,
  error: null,

  startAuth: async () => {
    set({ isLoading: true, error: null })
    try {
      const info = await tauriInvoke<DeviceFlowInfo>('cmd_start_auth')
      set({ deviceFlowInfo: info, authStep: 'waiting_device', isLoading: false })
    } catch (e) {
      const error = parseCommandError(String(e))
      set({ error: error.message, isLoading: false, authStep: 'idle' })
    }
  },

  pollAuth: async () => {
    const { deviceFlowInfo } = get()
    if (!deviceFlowInfo) return

    set({ authStep: 'polling' })

    try {
      const user = await tauriInvoke<AuthenticatedUser>('cmd_poll_auth', {
        deviceCode: deviceFlowInfo.device_code,
        interval: deviceFlowInfo.interval,
      })
      set({ user, authStep: 'authenticated', isLoading: false, deviceFlowInfo: null })
    } catch (e) {
      const error = parseCommandError(String(e))
      if (error.code === 'PENDING') {
        setTimeout(() => get().pollAuth(), deviceFlowInfo.interval * 1000)
      } else if (error.code === 'SLOW_DOWN') {
        setTimeout(() => get().pollAuth(), (deviceFlowInfo.interval + 5) * 1000)
      } else {
        set({ error: error.message, authStep: 'idle', isLoading: false })
      }
    }
  },

  checkExistingSession: async () => {
    set({ isLoading: true })
    try {
      const user = await tauriInvoke<AuthenticatedUser | null>('cmd_get_current_user')
      if (user) {
        set({ user, authStep: 'authenticated', isLoading: false })
      } else {
        set({ authStep: 'idle', isLoading: false })
      }
    } catch {
      set({ authStep: 'idle', isLoading: false })
    }
  },

  logout: async () => {
    const { user } = get()
    if (user) {
      try {
        await tauriInvoke('cmd_logout', { username: user.login })
      } catch {
        // Ignore logout errors
      }
    }
    set({ user: null, authStep: 'idle', deviceFlowInfo: null })
  },

  setUser: (user) => {
    set({ user, authStep: 'authenticated' })
  },

  clearError: () => set({ error: null }),
}))
