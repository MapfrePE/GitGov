import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { AuthenticatedUser, DeviceFlowInfo } from '@/lib/types'

type AuthStep = 'idle' | 'waiting_device' | 'polling' | 'authenticated'
const LEGACY_LOCAL_PIN_KEY = 'gitgov.local_pin_v1'
let authPollTimer: ReturnType<typeof setTimeout> | null = null
let authPollInFlight = false
let cachedLocalPin: string | null = null
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
  setLocalPin: (pin: string) => Promise<void>
  clearLocalPin: () => Promise<void>
  unlockWithPin: (pin: string) => boolean
  lockSession: () => void
}

function readLegacyStoredPin(): string | null {
  try {
    const value = localStorage.getItem(LEGACY_LOCAL_PIN_KEY)
    const normalized = value?.trim() ?? ''
    return isValidPin(normalized) ? normalized : null
  } catch {
    return null
  }
}

function clearLegacyStoredPin(): void {
  try {
    localStorage.removeItem(LEGACY_LOCAL_PIN_KEY)
  } catch {
    // ignore local storage errors
  }
}

function isValidPin(pin: string): boolean {
  return /^[0-9]{4,6}$/.test(pin.trim())
}

function normalizeSecurePin(value: string | null | undefined): string | null {
  const normalized = (value ?? '').trim()
  return isValidPin(normalized) ? normalized : null
}

async function loadSecurePin(): Promise<string | null> {
  try {
    const securePin = await tauriInvoke<string | null>('cmd_pin_get')
    return normalizeSecurePin(securePin)
  } catch {
    return null
  }
}

async function hydratePinFromSecureStorage(): Promise<string | null> {
  const securePin = await loadSecurePin()
  if (securePin) {
    cachedLocalPin = securePin
    clearLegacyStoredPin()
    return securePin
  }

  const legacyPin = readLegacyStoredPin()
  if (!legacyPin) {
    cachedLocalPin = null
    clearLegacyStoredPin()
    return null
  }

  // One-shot migration from localStorage into keyring; legacy key is removed either way.
  clearLegacyStoredPin()
  try {
    await tauriInvoke('cmd_pin_set', { pin: legacyPin })
    cachedLocalPin = legacyPin
    return legacyPin
  } catch {
    cachedLocalPin = null
    return null
  }
}

export const useAuthStore = create<AuthState & AuthActions>((set, get) => ({
  user: null,
  isLoading: false,
  authStep: 'idle',
  deviceFlowInfo: null,
  error: null,
  isPinEnabled: false,
  pinUnlocked: true,
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
    if (authPollInFlight) return
    if (authPollTimer) {
      clearTimeout(authPollTimer)
      authPollTimer = null
    }

    authPollInFlight = true
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
      const hasPin = (await hydratePinFromSecureStorage()) !== null
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
    } finally {
      authPollInFlight = false
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
    const hasPin = (await hydratePinFromSecureStorage()) !== null
    if (REQUIRE_DEVICE_FLOW_ON_START) {
      // Product decision: every desktop restart must pass through GitHub Device Flow.
      set({
        user: null,
        authStep: 'idle',
        deviceFlowInfo: null,
        isLoading: false,
        error: null,
        isPinEnabled: hasPin,
        pinUnlocked: true,
        pinError: null,
      })
      return
    }
    try {
      const user = await tauriInvoke<AuthenticatedUser | null>('cmd_get_current_user')
      if (user) {
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
    const hasPin = cachedLocalPin !== null
    set({ user, authStep: 'authenticated', isPinEnabled: hasPin, pinUnlocked: !hasPin, pinError: null })
  },

  clearError: () => set({ error: null }),

  setLocalPin: async (pin) => {
    const normalized = pin.trim()
    if (!isValidPin(normalized)) {
      set({ pinError: 'PIN inválido. Usa 4 a 6 dígitos.' })
      return
    }
    try {
      await tauriInvoke('cmd_pin_set', { pin: normalized })
      clearLegacyStoredPin()
      cachedLocalPin = normalized
      set({ isPinEnabled: true, pinUnlocked: true, pinError: null })
    } catch {
      set({ pinError: 'No se pudo guardar el PIN local.' })
    }
  },

  clearLocalPin: async () => {
    try {
      await tauriInvoke('cmd_pin_clear')
      clearLegacyStoredPin()
      cachedLocalPin = null
      set({ isPinEnabled: false, pinUnlocked: true, pinError: null })
    } catch {
      set({ pinError: 'No se pudo eliminar el PIN local.' })
    }
  },

  unlockWithPin: (pin) => {
    const stored = cachedLocalPin
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
    if (cachedLocalPin) {
      set({ pinUnlocked: false, isPinEnabled: true })
    }
  },
}))
