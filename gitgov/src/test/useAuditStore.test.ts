import { describe, it, expect, beforeEach, vi } from 'vitest'

const mockInvoke = vi.fn()

vi.mock('@/lib/tauri', () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
  parseCommandError: (error: string) => {
    try {
      const parsed = JSON.parse(error)
      return { code: parsed.code || 'UNKNOWN', message: parsed.message || error }
    } catch {
      return { code: 'UNKNOWN', message: error }
    }
  },
}))

import { useAuditStore } from '@/store/useAuditStore'

describe('useAuditStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuditStore.setState({
      logs: [],
      stats: null,
      filter: { limit: 100, offset: 0 },
      isLoading: false,
      error: null,
    })
  })

  describe('initial state', () => {
    it('starts with empty logs and default filter', () => {
      const state = useAuditStore.getState()
      expect(state.logs).toEqual([])
      expect(state.stats).toBeNull()
      expect(state.filter.limit).toBe(100)
      expect(state.filter.offset).toBe(0)
      expect(state.isLoading).toBe(false)
    })
  })

  describe('setFilter', () => {
    it('merges partial filter with existing', () => {
      useAuditStore.getState().setFilter({ developer_login: 'dev1' })
      expect(useAuditStore.getState().filter.developer_login).toBe('dev1')
      expect(useAuditStore.getState().filter.limit).toBe(100) // preserved
    })

    it('overwrites existing filter fields', () => {
      useAuditStore.getState().setFilter({ limit: 50 })
      expect(useAuditStore.getState().filter.limit).toBe(50)
      useAuditStore.getState().setFilter({ limit: 200 })
      expect(useAuditStore.getState().filter.limit).toBe(200)
    })

    it('can set multiple fields at once', () => {
      useAuditStore.getState().setFilter({
        developer_login: 'admin',
        action: 'Push',
        status: 'Success',
      })
      const filter = useAuditStore.getState().filter
      expect(filter.developer_login).toBe('admin')
      expect(filter.action).toBe('Push')
      expect(filter.status).toBe('Success')
    })
  })

  describe('loadLogs', () => {
    it('calls cmd_get_audit_logs with filter and isAdmin', async () => {
      const mockLogs = [
        { id: 1, timestamp: '2024-01-01', developer_login: 'dev1', action: 'Push', status: 'Success' },
      ]
      mockInvoke.mockResolvedValueOnce(mockLogs)

      await useAuditStore.getState().loadLogs(true)

      expect(mockInvoke).toHaveBeenCalledWith('cmd_get_audit_logs', {
        filter: { limit: 100, offset: 0 },
        isAdmin: true,
      })
      expect(useAuditStore.getState().logs).toEqual(mockLogs)
      expect(useAuditStore.getState().isLoading).toBe(false)
    })

    it('sets error on failure', async () => {
      mockInvoke.mockRejectedValueOnce(
        JSON.stringify({ code: 'DB', message: 'Database error' }),
      )

      await useAuditStore.getState().loadLogs(false)

      expect(useAuditStore.getState().error).toBe('Database error')
      expect(useAuditStore.getState().isLoading).toBe(false)
    })
  })

  describe('loadStats', () => {
    it('calls cmd_get_audit_stats and stores result', async () => {
      const mockStats = { pushes_today: 10, blocked_today: 2, active_devs_this_week: 5 }
      mockInvoke.mockResolvedValueOnce(mockStats)

      await useAuditStore.getState().loadStats()

      expect(useAuditStore.getState().stats).toEqual(mockStats)
    })

    it('sets error on failure', async () => {
      mockInvoke.mockRejectedValueOnce('stats error')

      await useAuditStore.getState().loadStats()

      expect(useAuditStore.getState().error).toBe('stats error')
    })
  })

  describe('loadMyLogs', () => {
    it('calls cmd_get_my_logs with login and default limit', async () => {
      const mockLogs = [{ id: 1, developer_login: 'dev1' }]
      mockInvoke.mockResolvedValueOnce(mockLogs)

      await useAuditStore.getState().loadMyLogs('dev1')

      expect(mockInvoke).toHaveBeenCalledWith('cmd_get_my_logs', {
        developerLogin: 'dev1',
        limit: 50,
      })
      expect(useAuditStore.getState().logs).toEqual(mockLogs)
    })

    it('accepts custom limit', async () => {
      mockInvoke.mockResolvedValueOnce([])

      await useAuditStore.getState().loadMyLogs('dev1', 10)

      expect(mockInvoke).toHaveBeenCalledWith('cmd_get_my_logs', {
        developerLogin: 'dev1',
        limit: 10,
      })
    })
  })

  describe('clearError', () => {
    it('clears error', () => {
      useAuditStore.setState({ error: 'an error' })
      useAuditStore.getState().clearError()
      expect(useAuditStore.getState().error).toBeNull()
    })
  })
})
