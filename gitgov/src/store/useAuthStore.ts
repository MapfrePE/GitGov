import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { AuthenticatedUser, DeviceFlowInfo } from '@/lib/types'

type AuthStep = 'idle' | 'waiting_device' | 'polling' | 'authenticated'
const LOCAL_PIN_KEY = 'gitgov.local_pin_v1'
let authPollTimer: ReturnType<typeof setTimeout> | null = null
const REQUIRE_DEVICE_FLOW_ON_START =
  String(import.meta.env.VITE_REQUIRE_DEVICE_FLOW_ON_START ?? 'true').toLowerCase() !== 'false'
const MIN_POLLING_VISUAL_MS = 900

interface AuthState {
  user: AuthenticatedUser | null
  isLoading: boolean
  authStep: AuthStep
  deviceFlowInfo: DeviceFlowInfo | null
  error: string | null
  isPinEnabled: boolean
  pinUnlocked: boolean
  pinError: string | null
}

interface AuthActions {
  startAuth: () => Promise<void>
  pollAuth: () => Promise<void>
  cancelAuth: () => void
  checkExistingSession: () => Promise<void>
  logout: () => Promise<void>
  setUser: (user: AuthenticatedUser) => void
  clearError: () => void
  setLocalPin: (pin: string) => void
  clearLocalPin: () => void
  unlockWithPin: (pin: string) => boolean
  lockSession: () => void
}

function getStoredPin(): string | null {
  try {
    return localStorage.getItem(LOCAL_PIN_KEY)
  } catch {
    return null
  }
}

function isValidPin(pin: string): boolean {
  return /^[0-9]{4,6}$/.test(pin.trim())
}

export const useAuthStore = create<AuthState & AuthActions>((set, get) => ({
  user: null,
  isLoading: false,
  authStep: 'idle',
  deviceFlowInfo: null,
  error: null,
  isPinEnabled: getStoredPin() !== null,
  pinUnlocked: getStoredPin() === null,
  pinError: null,

  startAuth: async () => {
    if (authPollTimer) {
      clearTimeout(authPollTimer)
      authPollTimer = null
    }
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
    if (authPollTimer) {
      clearTimeout(authPollTimer)
      authPollTimer = null
    }

    set({ authStep: 'polling' })

    try {
      const startedAt = Date.now()
      const user = await tauriInvoke<AuthenticatedUser>('cmd_poll_auth', {
        deviceCode: deviceFlowInfo.device_code,
        interval: deviceFlowInfo.interval,
      })
      const elapsed = Date.now() - startedAt
      if (elapsed < MIN_POLLING_VISUAL_MS) {
        await new Promise((resolve) => setTimeout(resolve, MIN_POLLING_VISUAL_MS - elapsed))
      }
      const hasPin = getStoredPin() !== null
      set({
        user,
        authStep: 'authenticated',
        isLoading: false,
        deviceFlowInfo: null,
        isPinEnabled: hasPin,
        pinUnlocked: true,
        pinError: null,
      })
    } catch (e) {
      const error = parseCommandError(String(e))
      if (error.code === 'PENDING') {
        if (authPollTimer) {
          clearTimeout(authPollTimer)
        }
        authPollTimer = setTimeout(() => {
          authPollTimer = null
          void get().pollAuth()
        }, deviceFlowInfo.interval * 1000)
      } else if (error.code === 'SLOW_DOWN') {
        if (authPollTimer) {
          clearTimeout(authPollTimer)
        }
        authPollTimer = setTimeout(() => {
          authPollTimer = null
          void get().pollAuth()
        }, (deviceFlowInfo.interval + 5) * 1000)
      } else {
        if (authPollTimer) {
          clearTimeout(authPollTimer)
          authPollTimer = null
        }
        set({ error: error.message, authStep: 'idle', isLoading: false })
      }
    }
  },

  cancelAuth: () => {
    if (authPollTimer) {
      clearTimeout(authPollTimer)
      authPollTimer = null
    }
    set({ authStep: 'idle', deviceFlowInfo: null, isLoading: false, error: null })
  },

  checkExistingSession: async () => {
    set({ isLoading: true })
    if (REQUIRE_DEVICE_FLOW_ON_START) {
      // Product decision: every desktop restart must pass through GitHub Device Flow.
      set({
        user: null,
        authStep: 'idle',
        deviceFlowInfo: null,
        isLoading: false,
        error: null,
        pinUnlocked: true,
        pinError: null,
      })
      return
    }
    try {
      const user = await tauriInvoke<AuthenticatedUser | null>('cmd_get_current_user')
      if (user) {
        const hasPin = getStoredPin() !== null
        set({
          user,
          authStep: 'authenticated',
          isLoading: false,
          isPinEnabled: hasPin,
          pinUnlocked: !hasPin,
          pinError: null,
        })
      } else {
        set({ authStep: 'idle', isLoading: false, pinUnlocked: true, pinError: null })
      }
    } catch {
      set({ authStep: 'idle', isLoading: false, pinUnlocked: true, pinError: null })
    }
  },

  logout: async () => {
    if (authPollTimer) {
      clearTimeout(authPollTimer)
      authPollTimer = null
    }
    const { user } = get()
    if (user) {
      try {
        await tauriInvoke('cmd_logout', { username: user.login })
      } catch {
        // Ignore logout errors
      }
    }
    set({ user: null, authStep: 'idle', deviceFlowInfo: null, pinUnlocked: false, pinError: null })
  },

  setUser: (user) => {
    const hasPin = getStoredPin() !== null
    set({ user, authStep: 'authenticated', isPinEnabled: hasPin, pinUnlocked: !hasPin, pinError: null })
  },

  clearError: () => set({ error: null }),

  setLocalPin: (pin) => {
    const normalized = pin.trim()
    if (!isValidPin(normalized)) {
      set({ pinError: 'PIN inválido. Usa 4 a 6 dígitos.' })
      return
    }
    try {
      localStorage.setItem(LOCAL_PIN_KEY, normalized)
      set({ isPinEnabled: true, pinUnlocked: true, pinError: null })
    } catch {
      set({ pinError: 'No se pudo guardar el PIN local.' })
    }
  },

  clearLocalPin: () => {
    try {
      localStorage.removeItem(LOCAL_PIN_KEY)
      set({ isPinEnabled: false, pinUnlocked: true, pinError: null })
    } catch {
      set({ pinError: 'No se pudo eliminar el PIN local.' })
    }
  },

  unlockWithPin: (pin) => {
    const stored = getStoredPin()
    if (!stored) {
      set({ pinUnlocked: true, pinError: null, isPinEnabled: false })
      return true
    }
    if (pin.trim() === stored) {
      set({ pinUnlocked: true, pinError: null, isPinEnabled: true })
      return true
    }
    set({ pinUnlocked: false, pinError: 'PIN incorrecto.' })
    return false
  },

  lockSession: () => {
    if (getStoredPin()) {
      set({ pinUnlocked: false, isPinEnabled: true })
    }
  },
}))
