import { describe, it, expect, vi } from 'vitest'

// Mock tauri helper
vi.mock('@/lib/tauri', () => ({
  isTauriDesktop: vi.fn().mockReturnValue(false),
}))

import {
  getDesktopUpdateFallbackUrl,
  canUseDesktopUpdater,
  isUpdaterNotConfiguredError,
  normalizeUpdaterErrorMessage,
} from '@/lib/updater'

describe('updater utility', () => {
  describe('getDesktopUpdateFallbackUrl', () => {
    it('returns GitHub releases URL by default', () => {
      const url = getDesktopUpdateFallbackUrl()
      expect(url).toContain('github.com/MapfrePE/GitGov/releases')
    })

    it('returns same URL for stable channel (no template)', () => {
      const url = getDesktopUpdateFallbackUrl('stable')
      expect(url).toContain('releases/latest')
    })

    it('returns same URL for beta channel (no template in default URL)', () => {
      const url = getDesktopUpdateFallbackUrl('beta')
      // Default URL ends with /releases/latest — no {channel} template
      expect(url).toContain('releases/latest')
    })
  })

  describe('canUseDesktopUpdater', () => {
    it('returns false when not in Tauri', () => {
      expect(canUseDesktopUpdater()).toBe(false)
    })
  })

  describe('isUpdaterNotConfiguredError', () => {
    it('detects updater config errors', () => {
      expect(isUpdaterNotConfiguredError('Updater endpoint not configured')).toBe(true)
      expect(isUpdaterNotConfiguredError('updater pubkey missing')).toBe(true)
      expect(isUpdaterNotConfiguredError('Updater config error')).toBe(true)
    })

    it('returns false for unrelated errors', () => {
      expect(isUpdaterNotConfiguredError('Network timeout')).toBe(false)
      expect(isUpdaterNotConfiguredError('Connection refused')).toBe(false)
      expect(isUpdaterNotConfiguredError(null)).toBe(false)
      expect(isUpdaterNotConfiguredError(undefined)).toBe(false)
    })

    it('handles non-string errors', () => {
      expect(isUpdaterNotConfiguredError(new Error('Updater endpoint missing'))).toBe(true)
      expect(isUpdaterNotConfiguredError(42)).toBe(false)
    })
  })

  describe('normalizeUpdaterErrorMessage', () => {
    it('returns user-friendly message for decoding error', () => {
      const msg = normalizeUpdaterErrorMessage('error decoding response body from server')
      expect(msg).toContain('latest.json')
      expect(msg).toContain('Descarga manual')
    })

    it('returns user-friendly message for 404 latest.json', () => {
      const msg = normalizeUpdaterErrorMessage('404 latest.json not found')
      expect(msg).toContain('latest.json')
    })

    it('returns user-friendly message for TLS errors', () => {
      const msg = normalizeUpdaterErrorMessage('TLS handshake failed')
      expect(msg).toContain('TLS')
    })

    it('returns user-friendly message for certificate errors', () => {
      const msg = normalizeUpdaterErrorMessage('certificate verification failed')
      expect(msg).toContain('certificado')
    })

    it('returns raw message for unknown errors', () => {
      const msg = normalizeUpdaterErrorMessage('Something unexpected happened')
      expect(msg).toBe('Something unexpected happened')
    })

    it('handles null/undefined', () => {
      // The mock returns String(e), so String(null) = '', String(undefined) = ''
      const resultNull = normalizeUpdaterErrorMessage(null)
      const resultUndefined = normalizeUpdaterErrorMessage(undefined)
      expect(typeof resultNull).toBe('string')
      expect(typeof resultUndefined).toBe('string')
    })
  })
})
