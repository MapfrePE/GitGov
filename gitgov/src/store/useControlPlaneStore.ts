import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { AuditLogEntry } from '@/lib/types'

interface ServerConfig {
  url: string
  api_key?: string
}

interface ServerStats {
  pushes_today: number
  blocked_today: number
  active_devs_this_week: number
  most_frequent_action?: string
  total_events: number
  events_by_repo: Record<string, number>
  events_by_developer: Record<string, number>
}

interface ControlPlaneState {
  serverConfig: ServerConfig | null
  serverStats: ServerStats | null
  serverLogs: AuditLogEntry[]
  isConnected: boolean
  isLoading: boolean
  error: string | null
}

interface ControlPlaneActions {
  setServerConfig: (config: ServerConfig) => void
  checkConnection: () => Promise<void>
  loadStats: () => Promise<void>
  loadLogs: (limit?: number) => Promise<void>
  clearError: () => void
  disconnect: () => void
}

export const useControlPlaneStore = create<ControlPlaneState & ControlPlaneActions>((set, get) => ({
  serverConfig: null,
  serverStats: null,
  serverLogs: [],
  isConnected: false,
  isLoading: false,
  error: null,

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
        filter: { limit },
      })
      set({ serverLogs: logs, isLoading: false })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false })
    }
  },

  clearError: () => set({ error: null }),

  disconnect: () => set({ serverConfig: null, isConnected: false, serverStats: null, serverLogs: [] }),
}))
