import { describe, it, expect, beforeEach, vi } from 'vitest'

// Mock dependencies
vi.mock('@/lib/tauri', () => ({
  isTauriDesktop: vi.fn().mockReturnValue(false),
}))

vi.mock('@/lib/updater', () => ({
  canUseDesktopUpdater: vi.fn().mockReturnValue(false),
  checkDesktopUpdate: vi.fn(),
  downloadAndInstallDesktopUpdate: vi.fn(),
  getDesktopUpdateFallbackUrl: vi.fn().mockReturnValue('https://github.com/MapfrePE/GitGov/releases/latest'),
  isUpdaterNotConfiguredError: vi.fn().mockReturnValue(false),
  normalizeUpdaterErrorMessage: vi.fn().mockImplementation((e: unknown) => String(e)),
}))

vi.mock('@/components/shared/Toast', () => ({
  toast: vi.fn(),
}))

import { useUpdateStore } from '@/store/useUpdateStore'

describe('useUpdateStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    useUpdateStore.setState({
      status: 'unsupported',
      isChecking: false,
      isDownloading: false,
      isUpdaterSupported: false,
      isUpdaterConfigured: true,
      updateInfo: null,
      progress: null,
      lastCheckedAt: null,
      error: null,
      channel: 'stable',
      fallbackDownloadUrl: 'https://github.com/MapfrePE/GitGov/releases/latest',
      changelogExpanded: false,
      _updateHandle: null,
    })
  })

  describe('initial state (non-Tauri)', () => {
    it('starts as unsupported when not in Tauri', () => {
      expect(useUpdateStore.getState().status).toBe('unsupported')
      expect(useUpdateStore.getState().isUpdaterSupported).toBe(false)
    })
  })

  describe('setChannel', () => {
    it('sets channel to beta', () => {
      useUpdateStore.getState().setChannel('beta')
      expect(useUpdateStore.getState().channel).toBe('beta')
    })

    it('sets channel to stable', () => {
      useUpdateStore.getState().setChannel('beta')
      useUpdateStore.getState().setChannel('stable')
      expect(useUpdateStore.getState().channel).toBe('stable')
    })

    it('persists channel to localStorage', () => {
      useUpdateStore.getState().setChannel('beta')
      expect(localStorage.getItem('gitgov:desktop-updater:channel')).toBe('beta')
    })
  })

  describe('clearError', () => {
    it('clears the error', () => {
      useUpdateStore.setState({ error: 'some error' })
      useUpdateStore.getState().clearError()
      expect(useUpdateStore.getState().error).toBeNull()
    })
  })

  describe('dismissUpdate', () => {
    it('clears update info and resets to idle', () => {
      useUpdateStore.setState({
        updateInfo: { currentVersion: '0.1.0', version: '0.2.0' },
        status: 'update-available',
        _updateHandle: { fake: true },
      })
      useUpdateStore.getState().dismissUpdate()

      expect(useUpdateStore.getState().updateInfo).toBeNull()
      expect(useUpdateStore.getState()._updateHandle).toBeNull()
      expect(useUpdateStore.getState().status).toBe('idle')
    })
  })

  describe('setChangelogExpanded', () => {
    it('sets changelog expanded state', () => {
      useUpdateStore.getState().setChangelogExpanded(true)
      expect(useUpdateStore.getState().changelogExpanded).toBe(true)
      useUpdateStore.getState().setChangelogExpanded(false)
      expect(useUpdateStore.getState().changelogExpanded).toBe(false)
    })
  })

  describe('checkForUpdates (unsupported)', () => {
    it('sets unsupported status when not in Tauri', async () => {
      await useUpdateStore.getState().checkForUpdates({ manual: true })
      expect(useUpdateStore.getState().status).toBe('unsupported')
      expect(useUpdateStore.getState().isUpdaterSupported).toBe(false)
    })
  })

  describe('downloadAndInstall', () => {
    it('does nothing when no update handle', async () => {
      useUpdateStore.setState({ _updateHandle: null, updateInfo: null })
      await useUpdateStore.getState().downloadAndInstall()
      expect(useUpdateStore.getState().isDownloading).toBe(false)
    })
  })
})
