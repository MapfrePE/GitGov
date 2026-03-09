import { describe, it, expect, beforeEach } from 'vitest'
import {
  detectBrowserTimezone,
  readStoredTimezone,
  persistTimezone,
  formatTs,
  formatTimeOnly,
  formatDateOnly,
  STORAGE_KEY,
  TIMEZONES,
} from '@/lib/timezone'

describe('timezone utility', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  describe('TIMEZONES', () => {
    it('has at least 10 timezone options', () => {
      expect(TIMEZONES.length).toBeGreaterThanOrEqual(10)
    })

    it('includes UTC as first option', () => {
      expect(TIMEZONES[0].value).toBe('UTC')
    })

    it('each timezone has label and value', () => {
      for (const tz of TIMEZONES) {
        expect(tz.label).toBeTruthy()
        expect(tz.value).toBeTruthy()
      }
    })
  })

  describe('detectBrowserTimezone', () => {
    it('returns a non-empty string', () => {
      const tz = detectBrowserTimezone()
      expect(typeof tz).toBe('string')
      expect(tz.length).toBeGreaterThan(0)
    })
  })

  describe('readStoredTimezone / persistTimezone', () => {
    it('returns null when nothing stored', () => {
      expect(readStoredTimezone()).toBeNull()
    })

    it('persists and reads a valid timezone', () => {
      persistTimezone('America/Lima')
      expect(readStoredTimezone()).toBe('America/Lima')
    })

    it('does not persist an invalid timezone', () => {
      persistTimezone('Invalid/Timezone_XXXX')
      expect(readStoredTimezone()).toBeNull()
    })

    it('returns null for stored invalid value', () => {
      localStorage.setItem(STORAGE_KEY, 'Not/A/Real/Zone')
      expect(readStoredTimezone()).toBeNull()
    })

    it('returns valid stored value', () => {
      localStorage.setItem(STORAGE_KEY, 'UTC')
      expect(readStoredTimezone()).toBe('UTC')
    })
  })

  describe('formatTs', () => {
    it('returns "—" for null', () => {
      expect(formatTs(null, 'UTC')).toBe('—')
    })

    it('returns "—" for undefined', () => {
      expect(formatTs(undefined, 'UTC')).toBe('—')
    })

    it('returns "—" for NaN epoch', () => {
      expect(formatTs(NaN, 'UTC')).toBe('—')
    })

    it('formats a valid epoch-ms in UTC', () => {
      // 2024-01-15T12:00:00.000Z
      const epoch = Date.UTC(2024, 0, 15, 12, 0, 0)
      const result = formatTs(epoch, 'UTC')
      expect(result).toContain('2024')
      expect(result).toContain('15')
    })

    it('accepts string date input', () => {
      const result = formatTs('2024-06-01T10:00:00Z', 'UTC')
      expect(result).toContain('2024')
    })

    it('returns "—" for unparseable string', () => {
      expect(formatTs('not-a-date', 'UTC')).toBe('—')
    })

    it('falls back gracefully for invalid timezone', () => {
      const epoch = Date.UTC(2024, 0, 15, 12, 0, 0)
      const result = formatTs(epoch, 'Invalid/TZ_DOES_NOT_EXIST')
      // Should not throw — falls back to toLocaleString
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    })
  })

  describe('formatTimeOnly', () => {
    it('returns "—" for null', () => {
      expect(formatTimeOnly(null, 'UTC')).toBe('—')
    })

    it('formats time for a valid epoch', () => {
      const epoch = Date.UTC(2024, 0, 15, 14, 30, 45)
      const result = formatTimeOnly(epoch, 'UTC')
      expect(result).toContain('14')
      expect(result).toContain('30')
      expect(result).toContain('45')
    })
  })

  describe('formatDateOnly', () => {
    it('returns "—" for null', () => {
      expect(formatDateOnly(null, 'UTC')).toBe('—')
    })

    it('formats date for a valid epoch', () => {
      const epoch = Date.UTC(2024, 5, 20, 12, 0, 0) // June 20, 2024
      const result = formatDateOnly(epoch, 'UTC')
      expect(result).toContain('2024')
      expect(result).toContain('20')
    })
  })
})
