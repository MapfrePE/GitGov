// ── Timezone utility for GitGov audit trail display ──────────────────────────
//
// All data is stored as UTC in PostgreSQL. This module converts to the
// user-selected display timezone for legal audit trail compliance.

export interface TimezoneOption {
  label: string
  value: string
}

export const TIMEZONES: TimezoneOption[] = [
  { label: 'UTC', value: 'UTC' },
  { label: 'Lima, Perú (UTC-5)', value: 'America/Lima' },
  { label: 'Bogotá, Colombia (UTC-5)', value: 'America/Bogota' },
  { label: 'Caracas, Venezuela (UTC-4)', value: 'America/Caracas' },
  { label: 'Santiago, Chile (UTC-4)', value: 'America/Santiago' },
  { label: 'Buenos Aires (UTC-3)', value: 'America/Argentina/Buenos_Aires' },
  { label: 'São Paulo, Brasil (UTC-3)', value: 'America/Sao_Paulo' },
  { label: 'Ciudad de México (UTC-6)', value: 'America/Mexico_City' },
  { label: 'Madrid, España (CET)', value: 'Europe/Madrid' },
  { label: 'Londres, UK (GMT)', value: 'Europe/London' },
  { label: 'Nueva York (ET)', value: 'America/New_York' },
  { label: 'Los Ángeles (PT)', value: 'America/Los_Angeles' },
]

export const STORAGE_KEY = 'gitgov:displayTimezone'

function isValidIanaTimezone(timezone: string): boolean {
  if (!timezone || typeof timezone !== 'string') return false
  try {
    // Throws RangeError for invalid IANA names.
    new Intl.DateTimeFormat('en-US', { timeZone: timezone })
    return true
  } catch {
    return false
  }
}

export function detectBrowserTimezone(): string {
  try {
    const tz = Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
    return isValidIanaTimezone(tz) ? tz : 'UTC'
  } catch {
    return 'UTC'
  }
}

export function readStoredTimezone(): string | null {
  try {
    if (typeof window === 'undefined' || !window.localStorage) return null
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    return isValidIanaTimezone(raw) ? raw : null
  } catch {
    return null
  }
}

export function persistTimezone(timezone: string): void {
  try {
    if (typeof window === 'undefined' || !window.localStorage) return
    if (!isValidIanaTimezone(timezone)) return
    window.localStorage.setItem(STORAGE_KEY, timezone)
  } catch {
    // ignore storage errors
  }
}

/** Format an epoch-ms timestamp as date + time in the given IANA timezone. */
export function formatTs(
  epochMs: number | string | null | undefined,
  timezone: string,
  options?: Intl.DateTimeFormatOptions,
): string {
  if (epochMs == null) return '—'
  const ms = typeof epochMs === 'string' ? Date.parse(epochMs) : epochMs
  if (!Number.isFinite(ms)) return '—'
  try {
    return new Intl.DateTimeFormat('es', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      timeZone: timezone,
      ...options,
    }).format(new Date(ms))
  } catch {
    // Fallback if timezone string is invalid
    return new Date(ms).toLocaleString()
  }
}

/** Format epoch-ms as HH:mm:ss only. */
export function formatTimeOnly(
  epochMs: number | null | undefined,
  timezone: string,
): string {
  if (epochMs == null) return '—'
  try {
    return new Intl.DateTimeFormat('es', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      timeZone: timezone,
    }).format(new Date(epochMs))
  } catch {
    return new Date(epochMs).toLocaleTimeString()
  }
}

/** Format epoch-ms as date only (no time). */
export function formatDateOnly(
  epochMs: number | null | undefined,
  timezone: string,
): string {
  if (epochMs == null) return '—'
  try {
    return new Intl.DateTimeFormat('es', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      timeZone: timezone,
    }).format(new Date(epochMs))
  } catch {
    return new Date(epochMs).toLocaleDateString()
  }
}
