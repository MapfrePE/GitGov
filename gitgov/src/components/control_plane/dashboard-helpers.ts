import type { CombinedEvent } from '@/lib/types'

export function readDetailString(log: CombinedEvent, key: string): string | null {
  const value = log.details?.[key]
  if (typeof value === 'string' && value.trim().length > 0) return value
  const metadata = log.details && typeof log.details === 'object' ? (log.details['metadata'] as Record<string, unknown> | undefined) : undefined
  const nested = metadata?.[key]
  if (typeof nested === 'string' && nested.trim().length > 0) return nested
  const legacyDetails = log.details && typeof log.details === 'object' ? (log.details['legacy_details'] as Record<string, unknown> | undefined) : undefined
  const legacyMetadata = legacyDetails && typeof legacyDetails === 'object' ? (legacyDetails['metadata'] as Record<string, unknown> | undefined) : undefined
  const nestedLegacy = legacyMetadata?.[key]
  return typeof nestedLegacy === 'string' && nestedLegacy.trim().length > 0 ? nestedLegacy : null
}

export function getLogDetailPreview(log: CombinedEvent): string | null {
  if (log.event_type === 'commit') return readDetailString(log, 'commit_message')
  if (log.status === 'failed' || log.status === 'blocked') return readDetailString(log, 'reason')
  return null
}

export function getShortCommitSha(log: CombinedEvent): string | null {
  const sha = readDetailString(log, 'commit_sha')
  return sha ? sha.slice(0, 7) : null
}

export function extractTicketIdsFromCommitLog(log: CombinedEvent): string[] {
  const values = [readDetailString(log, 'commit_message'), log.branch ?? null].filter((v): v is string => typeof v === 'string' && v.trim().length > 0)
  const regex = /\b([A-Z][A-Z0-9]{1,15}-\d{1,9})\b/g
  const result: string[] = []
  const seen = new Set<string>()
  for (const value of values) {
    let match: RegExpExecArray | null
    regex.lastIndex = 0
    while ((match = regex.exec(value)) !== null) {
      const ticket = match[1].toUpperCase()
      if (!seen.has(ticket)) { seen.add(ticket); result.push(ticket) }
    }
  }
  return result
}

export function formatDurationMs(ms?: number): string {
  if (!ms || ms <= 0) return '-'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes <= 0) return `${seconds}s`
  return `${minutes}m ${seconds}s`
}

export function readDetailFiles(log: CombinedEvent): string[] {
  const direct = log.details?.['files']
  if (Array.isArray(direct)) return direct.filter((v): v is string => typeof v === 'string')
  return []
}

export interface DashboardRow { log: CombinedEvent; attachedFiles: string[] }

export function buildDashboardRows(logs: CombinedEvent[]): DashboardRow[] {
  const WINDOW_MS = 10 * 60 * 1000
  const rowsAscending: DashboardRow[] = []
  const pendingStageByUser = new Map<string, Array<{ created_at: number; files: string[] }>>()

  // Process oldest -> newest so each commit can consume the closest prior stage_files.
  for (let idx = logs.length - 1; idx >= 0; idx--) {
    const log = logs[idx]
    const login = (log.user_login ?? '').trim()

    if (log.event_type === 'stage_files') {
      if (!login) continue
      const files = readDetailFiles(log)
      if (!files.length) continue
      const queue = pendingStageByUser.get(login) ?? []
      queue.push({ created_at: log.created_at, files })
      pendingStageByUser.set(login, queue)
      continue
    }

    if (log.event_type !== 'commit') continue

    let attachedFiles: string[] = []
    if (login) {
      const queue = pendingStageByUser.get(login)
      if (queue && queue.length > 0) {
        // Drop stale candidates that are too old for this commit.
        while (queue.length > 0 && (log.created_at - queue[0].created_at) > WINDOW_MS) {
          queue.shift()
        }
        if (queue.length > 0) {
          const candidate = queue.pop()
          if (candidate && log.created_at >= candidate.created_at && (log.created_at - candidate.created_at) <= WINDOW_MS) {
            attachedFiles = candidate.files
          }
        }
        if (!queue.length) pendingStageByUser.delete(login)
      }
    }

    rowsAscending.push({ log, attachedFiles })
  }

  return rowsAscending.reverse()
}
