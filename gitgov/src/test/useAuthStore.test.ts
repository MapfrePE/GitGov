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

import { useAuthStore } from '@/store/useAuthStore'

describe('useAuthStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    // Reset store to initial state
    useAuthStore.setState({
      user: null,
      isLoading: false,
      authStep: 'idle',
      deviceFlowInfo: null,
      error: null,
      isPinEnabled: false,
      pinUnlocked: true,
      pinError: null,
    })
  })

  describe('initial state', () => {
    it('starts with no user and idle step', () => {
      const state = useAuthStore.getState()
      expect(state.user).toBeNull()
      expect(state.authStep).toBe('idle')
      expect(state.isLoading).toBe(false)
      expect(state.error).toBeNull()
    })

    it('starts with pin unlocked', () => {
      expect(useAuthStore.getState().pinUnlocked).toBe(true)
    })
  })

  describe('startAuth', () => {
    it('sets loading and clears error', async () => {
      useAuthStore.setState({ error: 'old error' })
      mockInvoke.mockResolvedValueOnce({
        device_code: 'abc',
        user_code: 'ABCD-1234',
        verification_uri: 'https://github.com/login/device',
        interval: 5,
        expires_in: 900,
      })

      await useAuthStore.getState().startAuth()

      expect(useAuthStore.getState().error).toBeNull()
      expect(useAuthStore.getState().authStep).toBe('waiting_device')
      expect(useAuthStore.getState().deviceFlowInfo).not.toBeNull()
      expect(useAuthStore.getState().deviceFlowInfo!.user_code).toBe('ABCD-1234')
    })

    it('sets error on failure', async () => {
      mockInvoke.mockRejectedValueOnce(
        JSON.stringify({ code: 'NETWORK', message: 'No internet' }),
      )

      await useAuthStore.getState().startAuth()

      expect(useAuthStore.getState().error).toBe('No internet')
      expect(useAuthStore.getState().authStep).toBe('idle')
      expect(useAuthStore.getState().isLoading).toBe(false)
    })
  })

  describe('cancelAuth', () => {
    it('resets to idle and clears device flow info', () => {
      useAuthStore.setState({
        authStep: 'polling',
        deviceFlowInfo: { device_code: 'x', user_code: 'Y', verification_uri: 'z', interval: 5 },
        error: 'some error',
      })

      useAuthStore.getState().cancelAuth()

      expect(useAuthStore.getState().authStep).toBe('idle')
      expect(useAuthStore.getState().deviceFlowInfo).toBeNull()
      expect(useAuthStore.getState().error).toBeNull()
    })
  })

  describe('logout', () => {
    it('clears user and resets to idle', async () => {
      useAuthStore.setState({
        user: { login: 'testuser', name: 'Test', avatar_url: '', is_admin: false },
        authStep: 'authenticated',
      })
      mockInvoke.mockResolvedValueOnce(undefined) // cmd_logout

      await useAuthStore.getState().logout()

      expect(useAuthStore.getState().user).toBeNull()
      expect(useAuthStore.getState().authStep).toBe('idle')
    })

    it('handles logout error gracefully', async () => {
      useAuthStore.setState({
        user: { login: 'testuser', name: 'Test', avatar_url: '', is_admin: false },
        authStep: 'authenticated',
      })
      mockInvoke.mockRejectedValueOnce(new Error('logout failed'))

      await useAuthStore.getState().logout()

      // Should still clear user even if cmd_logout fails
      expect(useAuthStore.getState().user).toBeNull()
      expect(useAuthStore.getState().authStep).toBe('idle')
    })
  })

  describe('setUser', () => {
    it('sets user and marks authenticated', () => {
      const user = { login: 'dev1', name: 'Developer', avatar_url: 'https://avatar', is_admin: true }
      useAuthStore.getState().setUser(user)

      expect(useAuthStore.getState().user).toEqual(user)
      expect(useAuthStore.getState().authStep).toBe('authenticated')
    })
  })

  describe('clearError', () => {
    it('clears error', () => {
      useAuthStore.setState({ error: 'something' })
      useAuthStore.getState().clearError()
      expect(useAuthStore.getState().error).toBeNull()
    })
  })

  describe('PIN management', () => {
    describe('setLocalPin', () => {
      it('rejects invalid PIN (too short)', async () => {
        await useAuthStore.getState().setLocalPin('12')
        expect(useAuthStore.getState().pinError).toContain('4 a 6 dígitos')
        expect(useAuthStore.getState().isPinEnabled).toBe(false)
      })

      it('rejects invalid PIN (letters)', async () => {
        await useAuthStore.getState().setLocalPin('abcd')
        expect(useAuthStore.getState().pinError).toContain('4 a 6 dígitos')
      })

      it('rejects invalid PIN (too long)', async () => {
        await useAuthStore.getState().setLocalPin('1234567')
        expect(useAuthStore.getState().pinError).toContain('4 a 6 dígitos')
      })

      it('accepts valid 4-digit PIN', async () => {
        mockInvoke.mockResolvedValueOnce(undefined) // cmd_pin_set
        await useAuthStore.getState().setLocalPin('1234')
        expect(useAuthStore.getState().isPinEnabled).toBe(true)
        expect(useAuthStore.getState().pinError).toBeNull()
      })

      it('accepts valid 6-digit PIN', async () => {
        mockInvoke.mockResolvedValueOnce(undefined) // cmd_pin_set
        await useAuthStore.getState().setLocalPin('123456')
        expect(useAuthStore.getState().isPinEnabled).toBe(true)
      })

      it('handles storage failure', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('keyring error'))
        await useAuthStore.getState().setLocalPin('1234')
        expect(useAuthStore.getState().pinError).toContain('No se pudo guardar')
      })
    })

    describe('clearLocalPin', () => {
      it('clears pin and disables', async () => {
        useAuthStore.setState({ isPinEnabled: true })
        mockInvoke.mockResolvedValueOnce(undefined) // cmd_pin_clear
        await useAuthStore.getState().clearLocalPin()
        expect(useAuthStore.getState().isPinEnabled).toBe(false)
        expect(useAuthStore.getState().pinUnlocked).toBe(true)
      })
    })

    describe('unlockWithPin', () => {
      it('returns true when no pin is set', () => {
        const result = useAuthStore.getState().unlockWithPin('anything')
        expect(result).toBe(true)
        expect(useAuthStore.getState().pinUnlocked).toBe(true)
      })
    })

    describe('lockSession', () => {
      it('does nothing when no pin is cached', () => {
        useAuthStore.setState({ pinUnlocked: true })
        useAuthStore.getState().lockSession()
        // No cached pin → should stay unlocked (lockSession only locks if cachedLocalPin exists)
        expect(useAuthStore.getState().pinUnlocked).toBe(true)
      })
    })
  })
})
