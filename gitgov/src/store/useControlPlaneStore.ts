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

// Default server config from environment
const DEFAULT_SERVER_CONFIG: ServerConfig = {
  url: 'http://localhost:3000',
  api_key: '57f1ed59-371d-46ef-9fdf-508f59bc4963',
}

export const useControlPlaneStore = create<ControlPlaneState & ControlPlaneActions>((set, get) => ({
  serverConfig: null,
  serverStats: null,
  serverLogs: [],
  isConnected: false,
  isLoading: false,
  error: null,

  initFromEnv: async () => {
    // Auto-connect with default config from .env
    set({ serverConfig: DEFAULT_SERVER_CONFIG })
    await get().checkConnection()
  },

  setServerConfig: (config) => {
    set({ serverConfig: config })
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
      const logs = await tauriInvoke<AuditLogEntry[]>('cmd_server_get_logs', {
        config: serverConfig,
        filter: { limit, offset: 0 },
      })
      set({ serverLogs: logs, isLoading: false })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false })
    }
  },

  clearError: () => set({ error: null }),

  disconnect: () => set({ serverConfig: null, isConnected: false, serverStats: null, serverLogs: [] }),
}))
