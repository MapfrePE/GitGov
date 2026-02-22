import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type { AuditLogEntry, AuditStats, AuditFilter } from '@/lib/types'

interface AuditState {
  logs: AuditLogEntry[]
  stats: AuditStats | null
  filter: AuditFilter
  isLoading: boolean
  error: string | null
}

interface AuditActions {
  loadLogs: (isAdmin: boolean) => Promise<void>
  loadStats: () => Promise<void>
  setFilter: (filter: Partial<AuditFilter>) => void
  loadMyLogs: (login: string, limit?: number) => Promise<void>
  clearError: () => void
}

const defaultFilter: AuditFilter = {
  limit: 100,
  offset: 0,
}

export const useAuditStore = create<AuditState & AuditActions>((set, get) => ({
  logs: [],
  stats: null,
  filter: defaultFilter,
  isLoading: false,
  error: null,

  loadLogs: async (isAdmin: boolean) => {
    set({ isLoading: true, error: null })
    try {
      const { filter } = get()
      const logs = await tauriInvoke<AuditLogEntry[]>('cmd_get_audit_logs', {
        filter,
        isAdmin,
      })
      set({ logs, isLoading: false })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false })
    }
  },

  loadStats: async () => {
    try {
      const stats = await tauriInvoke<AuditStats>('cmd_get_audit_stats')
      set({ stats })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  setFilter: (newFilter: Partial<AuditFilter>) => {
    const { filter } = get()
    set({ filter: { ...filter, ...newFilter } })
  },

  loadMyLogs: async (login: string, limit = 50) => {
    set({ isLoading: true, error: null })
    try {
      const logs = await tauriInvoke<AuditLogEntry[]>('cmd_get_my_logs', {
        developerLogin: login,
        limit,
      })
      set({ logs, isLoading: false })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, isLoading: false })
    }
  },

  clearError: () => set({ error: null }),
}))
