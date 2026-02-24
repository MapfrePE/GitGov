import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { CombinedEvent } from '@/lib/types'

interface ServerConfig {
  url: string
  api_key?: string
}

interface GitHubEventStats {
  total: number
  today: number
  pushes_today: number
  by_type: Record<string, number>
}

interface ClientEventStats {
  total: number
  today: number
  blocked_today: number
  by_type: Record<string, number>
  by_status: Record<string, number>
}

interface ViolationStats {
  total: number
  unresolved: number
  critical: number
}

interface ServerStats {
  github_events: GitHubEventStats
  client_events: ClientEventStats
  violations: ViolationStats
  active_devs_week: number
  active_repos: number
}

interface ControlPlaneState {
  serverConfig: ServerConfig | null
  serverStats: ServerStats | null
  serverLogs: CombinedEvent[]
  isConnected: boolean
  isLoading: boolean
  error: string | null
}

interface ControlPlaneActions {
  initFromEnv: () => Promise<void>
  setServerConfig: (config: ServerConfig) => void
  checkConnection: () => Promise<void>
  loadStats: () => Promise<void>
  loadLogs: (limit?: number) => Promise<void>
  clearError: () => void
  disconnect: () => void
}

const CONTROL_PLANE_CONFIG_STORAGE_KEY = 'gitgov.control_plane_config'

// Compatibility fallback: existing desktop setups relied on this default key.
// Keep it as last-resort fallback so the dashboard/logs continue working.
const LEGACY_DEFAULT_API_KEY = '57f1ed59-371d-46ef-9fdf-508f59bc4963'

function readStoredServerConfig(): ServerConfig | null {
  try {
    const raw = window.localStorage.getItem(CONTROL_PLANE_CONFIG_STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<ServerConfig>
    if (!parsed || typeof parsed.url !== 'string') return null
    return {
      url: parsed.url,
      api_key: typeof parsed.api_key === 'string' && parsed.api_key.trim() ? parsed.api_key : undefined,
    }
  } catch {
    return null
  }
}

function persistServerConfig(config: ServerConfig | null) {
  try {
    if (!config) {
      window.localStorage.removeItem(CONTROL_PLANE_CONFIG_STORAGE_KEY)
      return
    }
    window.localStorage.setItem(CONTROL_PLANE_CONFIG_STORAGE_KEY, JSON.stringify(config))
  } catch {
    // ignore storage errors
  }
}

function resolveServerConfig(input?: Partial<ServerConfig> | null, previous?: ServerConfig | null): ServerConfig {
  const stored = readStoredServerConfig()
  const url =
    input?.url?.trim() ||
    previous?.url?.trim() ||
    stored?.url?.trim() ||
    import.meta.env.VITE_SERVER_URL ||
    'http://localhost:3000'

  const apiKey =
    input?.api_key?.trim() ||
    previous?.api_key?.trim() ||
    stored?.api_key?.trim() ||
    import.meta.env.VITE_API_KEY ||
    LEGACY_DEFAULT_API_KEY

  return {
    url,
    api_key: apiKey || undefined,
  }
}

export const useControlPlaneStore = create<ControlPlaneState & ControlPlaneActions>((set, get) => ({
  serverConfig: null,
  serverStats: null,
  serverLogs: [],
  isConnected: false,
  isLoading: false,
  error: null,

  initFromEnv: async () => {
    // Auto-connect with stored config, env vars, or compatibility fallback.
    const config = resolveServerConfig()
    persistServerConfig(config)
    set({ serverConfig: config })
    await get().checkConnection()
  },

  setServerConfig: (config) => {
    const merged = resolveServerConfig(config, get().serverConfig)
    persistServerConfig(merged)
    set({ serverConfig: merged })
    get().checkConnection()
  },

  checkConnection: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return

    set({ isLoading: true, error: null })
    try {
      const healthy = await tauriInvoke<boolean>('cmd_server_health', { config: serverConfig })
      set({ isConnected: healthy, isLoading: false })
      if (healthy) {
        get().loadStats()
      }
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false, isConnected: false })
    }
  },

  loadStats: async () => {
    const { serverConfig } = get()
    if (!serverConfig) return

    try {
      const stats = await tauriInvoke<ServerStats>('cmd_server_get_stats', { config: serverConfig })
      set({ serverStats: stats })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  loadLogs: async (limit = 100) => {
    const { serverConfig } = get()
    if (!serverConfig) return

    set({ isLoading: true })
    try {
      const logs = await tauriInvoke<CombinedEvent[]>('cmd_server_get_logs', {
        config: serverConfig,
        filter: { limit, offset: 0 },
      })
      set({ serverLogs: logs, isLoading: false })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false })
    }
  },

  clearError: () => set({ error: null }),

  disconnect: () => {
    persistServerConfig(null)
    set({ serverConfig: null, isConnected: false, serverStats: null, serverLogs: [], error: null })
  },
}))
