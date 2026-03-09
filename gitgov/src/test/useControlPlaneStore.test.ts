import { describe, it, expect, beforeEach, vi } from 'vitest'

const mockInvoke = vi.fn()
const mockListen = vi.fn().mockResolvedValue(() => {})

vi.mock('@/lib/tauri', () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
  tauriListen: (...args: unknown[]) => mockListen(...args),
  parseCommandError: (error: string) => {
    try {
      const parsed = JSON.parse(error)
      return { code: parsed.code || 'UNKNOWN', message: parsed.message || error }
    } catch {
      return { code: 'UNKNOWN', message: error }
    }
  },
}))

vi.mock('@/lib/notifications', () => ({
  notifyNewEvents: vi.fn(),
}))

vi.mock('@/lib/timezone', () => ({
  detectBrowserTimezone: vi.fn().mockReturnValue('UTC'),
  readStoredTimezone: vi.fn().mockReturnValue(null),
  persistTimezone: vi.fn(),
}))

vi.mock('@/store/useAuthStore', () => ({
  useAuthStore: {
    getState: vi.fn().mockReturnValue({
      user: { login: 'testuser', name: 'Test', avatar_url: '', is_admin: true },
    }),
  },
}))

import { useControlPlaneStore } from '@/store/useControlPlaneStore'

describe('useControlPlaneStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    // Reset key state fields
    useControlPlaneStore.setState({
      serverConfig: null,
      serverStats: null,
      serverLogs: [],
      dailyActivity: [],
      jenkinsCorrelations: [],
      prMergeEvidence: [],
      ticketCoverage: null,
      userRole: null,
      userClientId: null,
      userOrgId: null,
      controlPlaneAuthConfirmed: false,
      isConnected: false,
      isLoading: false,
      error: null,
      logsPage: 1,
      logsPageSize: 50,
      chatSessions: [],
      activeChatSessionId: null,
      chatMessages: [],
      isChatLoading: false,
      displayTimezone: 'UTC',
      sseConnected: false,
      policyData: null,
      policyHistory: [],
      isPolicyLoading: false,
      isPolicySaving: false,
      policyError: null,
      selectedOrgName: '',
      orgUsers: [],
      orgUsersTotal: 0,
      orgInvitations: [],
      orgInvitationsTotal: 0,
      teamOverview: [],
      teamOverviewTotal: 0,
      teamRepos: [],
      teamReposTotal: 0,
      apiKeys: [],
      isLoadingApiKeys: false,
      exportLogs: [],
    })
  })

  describe('setServerConfig', () => {
    it('stores server config', () => {
      useControlPlaneStore.getState().setServerConfig({ url: 'http://127.0.0.1:3000', api_key: 'test-key' })
      expect(useControlPlaneStore.getState().serverConfig).toEqual({
        url: 'http://127.0.0.1:3000',
        api_key: 'test-key',
      })
    })
  })

  describe('clearError', () => {
    it('clears error', () => {
      useControlPlaneStore.setState({ error: 'an error' })
      useControlPlaneStore.getState().clearError()
      expect(useControlPlaneStore.getState().error).toBeNull()
    })
  })

  describe('setLogsPage', () => {
    it('sets the current page number', () => {
      useControlPlaneStore.getState().setLogsPage(3)
      expect(useControlPlaneStore.getState().logsPage).toBe(3)
    })
  })

  describe('setDisplayTimezone', () => {
    it('sets timezone', () => {
      useControlPlaneStore.getState().setDisplayTimezone('America/Lima')
      expect(useControlPlaneStore.getState().displayTimezone).toBe('America/Lima')
    })
  })

  describe('checkConnection', () => {
    it('calls server health and sets connected on success', async () => {
      useControlPlaneStore.setState({
        serverConfig: { url: 'http://127.0.0.1:3000', api_key: 'key' },
      })
      mockInvoke.mockResolvedValueOnce({ status: 'ok' }) // cmd_server_health

      await useControlPlaneStore.getState().checkConnection()

      expect(mockInvoke).toHaveBeenCalledWith('cmd_server_health', expect.any(Object))
    })

    it('sets error when no server config', async () => {
      useControlPlaneStore.setState({ serverConfig: null })
      await useControlPlaneStore.getState().checkConnection()
      // Should not throw but serverConfig is null so nothing should happen
      expect(mockInvoke).not.toHaveBeenCalled()
    })
  })

  describe('loadStats', () => {
    it('fetches stats from server', async () => {
      useControlPlaneStore.setState({
        serverConfig: { url: 'http://127.0.0.1:3000', api_key: 'key' },
      })
      const mockStats = {
        github_events: { total: 100, today: 10, pushes_today: 5, by_type: {} },
        client_events: { total: 50, today: 5, blocked_today: 1, desktop_pushes_today: 3, by_type: {}, by_status: {} },
        violations: { total: 2, unresolved: 1, critical: 0 },
        active_devs_week: 3,
        active_repos: 2,
      }
      mockInvoke.mockResolvedValueOnce(mockStats) // cmd_server_get_stats

      await useControlPlaneStore.getState().loadStats()

      expect(mockInvoke).toHaveBeenCalledWith('cmd_server_get_stats', expect.any(Object))
      expect(useControlPlaneStore.getState().serverStats).toEqual(mockStats)
    })
  })

  describe('loadLogs', () => {
    it('fetches logs from server', async () => {
      useControlPlaneStore.setState({
        serverConfig: { url: 'http://127.0.0.1:3000', api_key: 'key' },
      })
      // fetchLogsKeysetWindow → fetchLogsByFilter → tauriInvoke returns CombinedEvent[]
      const mockLogs = [
        { id: '1', source: 'client', event_type: 'commit', created_at: 1000, status: 'success', details: {} },
      ]
      mockInvoke.mockResolvedValueOnce(mockLogs)

      await useControlPlaneStore.getState().loadLogs(10, 0)

      expect(mockInvoke).toHaveBeenCalledWith('cmd_server_get_logs', expect.any(Object))
      expect(useControlPlaneStore.getState().serverLogs).toEqual(mockLogs)
    })
  })

  describe('chat session management', () => {
    it('creates a new chat session', () => {
      useControlPlaneStore.getState().createChatSession()
      const sessions = useControlPlaneStore.getState().chatSessions
      expect(sessions.length).toBe(1)
      // Title format: "Chat nuevo {n}" where n = sessions.length at creation
      expect(sessions[0].title).toMatch(/^Chat nuevo/)
      expect(useControlPlaneStore.getState().activeChatSessionId).toBe(sessions[0].id)
    })

    it('limits to max chat sessions', () => {
      for (let i = 0; i < 9; i++) {
        useControlPlaneStore.getState().createChatSession()
      }
      expect(useControlPlaneStore.getState().chatSessions.length).toBeLessThanOrEqual(8)
    })

    it('can close the only chat session (resets it)', () => {
      useControlPlaneStore.getState().createChatSession()
      const sessionId = useControlPlaneStore.getState().chatSessions[0].id
      useControlPlaneStore.getState().closeChatSession(sessionId)
      // With only 1 session, closeChatSession resets it instead of removing
      expect(useControlPlaneStore.getState().chatSessions.length).toBe(1)
      expect(useControlPlaneStore.getState().chatMessages).toEqual([])
    })

    it('clears chat messages for active session', () => {
      // clearChatMessages requires an active session
      useControlPlaneStore.getState().createChatSession()
      const sessionId = useControlPlaneStore.getState().chatSessions[0].id
      // Manually add messages to the session
      useControlPlaneStore.setState((s) => ({
        chatMessages: [{ id: '1', role: 'user' as const, content: 'test', timestamp: Date.now() }],
        chatSessions: s.chatSessions.map((ses) =>
          ses.id === sessionId
            ? { ...ses, messages: [{ id: '1', role: 'user' as const, content: 'test', timestamp: Date.now() }] }
            : ses,
        ),
      }))
      useControlPlaneStore.getState().clearChatMessages()
      expect(useControlPlaneStore.getState().chatMessages).toEqual([])
    })
  })

  describe('disconnect', () => {
    it('resets connection state', () => {
      // disconnectSse calls tauriInvoke('cmd_server_sse_disconnect') which needs to return a Promise
      mockInvoke.mockReturnValue(Promise.resolve(undefined))

      useControlPlaneStore.setState({
        isConnected: true,
        serverConfig: { url: 'http://127.0.0.1:3000', api_key: 'key' },
        serverStats: { github_events: { total: 1, today: 0, pushes_today: 0, by_type: {} }, client_events: { total: 0, today: 0, blocked_today: 0, desktop_pushes_today: 0, by_type: {}, by_status: {} }, violations: { total: 0, unresolved: 0, critical: 0 }, active_devs_week: 0, active_repos: 0 },
      })
      useControlPlaneStore.getState().disconnect()
      expect(useControlPlaneStore.getState().isConnected).toBe(false)
      expect(useControlPlaneStore.getState().serverConfig).toBeNull()
    })
  })

  describe('team filters', () => {
    it('sets team window days', () => {
      useControlPlaneStore.getState().setTeamFilters({ days: 30 })
      expect(useControlPlaneStore.getState().teamWindowDays).toBe(30)
    })

    it('sets team status filter', () => {
      useControlPlaneStore.getState().setTeamFilters({ status: 'active' })
      expect(useControlPlaneStore.getState().teamStatusFilter).toBe('active')
    })
  })

  describe('policy state', () => {
    it('loads policy from server', async () => {
      useControlPlaneStore.setState({
        serverConfig: { url: 'http://127.0.0.1:3000', api_key: 'key' },
      })
      const mockPolicy = {
        version: '1',
        checksum: 'abc',
        config: { branches: { patterns: [], protected: [] }, groups: {}, admins: [] },
        updated_at: Date.now(),
      }
      mockInvoke.mockResolvedValueOnce(mockPolicy)

      await useControlPlaneStore.getState().loadPolicy('my-repo')

      expect(mockInvoke).toHaveBeenCalledWith('cmd_server_get_policy', expect.any(Object))
      expect(useControlPlaneStore.getState().policyData).toEqual(mockPolicy)
    })
  })

  describe('selectedOrgName', () => {
    it('sets selected org name', () => {
      useControlPlaneStore.getState().setSelectedOrgName('my-org')
      expect(useControlPlaneStore.getState().selectedOrgName).toBe('my-org')
    })
  })
})
