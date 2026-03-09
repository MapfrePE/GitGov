import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import { isTauriDesktop } from './tauri'

// ── Notification preferences (persisted in localStorage) ──────────────

const PREFS_KEY = 'gitgov:notification-prefs'

export interface NotificationPrefs {
  enabled: boolean
  onNewEvents: boolean
  onBlockedPush: boolean
  onGovernanceWarn: boolean
}

const DEFAULT_PREFS: NotificationPrefs = {
  enabled: true,
  onNewEvents: true,
  onBlockedPush: true,
  onGovernanceWarn: true,
}

export function loadNotificationPrefs(): NotificationPrefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY)
    if (!raw) return { ...DEFAULT_PREFS }
    return { ...DEFAULT_PREFS, ...JSON.parse(raw) }
  } catch {
    return { ...DEFAULT_PREFS }
  }
}

export function saveNotificationPrefs(prefs: NotificationPrefs): void {
  localStorage.setItem(PREFS_KEY, JSON.stringify(prefs))
}

// ── Permission handling ──────────────────────────────────────────────

let permissionChecked = false
let permissionGranted = false

async function ensurePermission(): Promise<boolean> {
  if (!isTauriDesktop()) return false
  if (permissionChecked) return permissionGranted

  try {
    permissionGranted = await isPermissionGranted()
    if (!permissionGranted) {
      const result = await requestPermission()
      permissionGranted = result === 'granted'
    }
    permissionChecked = true
    return permissionGranted
  } catch {
    permissionChecked = true
    permissionGranted = false
    return false
  }
}

// ── Cooldown for new-events notifications (max 1 per 60s) ────────────

let lastNewEventsNotifyTs = 0
const NEW_EVENTS_COOLDOWN_MS = 60_000

// ── Safe send helper ────────────────────────────────────────────────

function safeSend(title: string, body: string): void {
  try {
    sendNotification({ title, body })
  } catch {
    // Fire-and-forget — swallow errors from OS notification layer
  }
}

// ── Public notification senders ─────────────────────────────────────

export async function notifyNewEvents(count: number): Promise<void> {
  const prefs = loadNotificationPrefs()
  if (!prefs.enabled || !prefs.onNewEvents) return

  // Cooldown: avoid spamming during heavy activity
  const now = Date.now()
  if (now - lastNewEventsNotifyTs < NEW_EVENTS_COOLDOWN_MS) return
  lastNewEventsNotifyTs = now

  if (!(await ensurePermission())) return

  safeSend(
    'GitGov — Nuevos eventos',
    `${count} nuevo${count === 1 ? '' : 's'} evento${count === 1 ? '' : 's'} registrado${count === 1 ? '' : 's'} en el Control Plane.`,
  )
}

export async function notifyBlockedPush(branch: string, reason: string): Promise<void> {
  const prefs = loadNotificationPrefs()
  if (!prefs.enabled || !prefs.onBlockedPush) return
  if (!(await ensurePermission())) return

  safeSend(
    'GitGov — Push bloqueado',
    `Push a "${branch}" fue bloqueado: ${reason}`,
  )
}

export async function notifyGovernanceWarning(warnings: string[]): Promise<void> {
  const prefs = loadNotificationPrefs()
  if (!prefs.enabled || !prefs.onGovernanceWarn) return
  if (!(await ensurePermission())) return

  safeSend(
    'GitGov — Advertencia de gobernanza',
    warnings.length === 1 ? warnings[0] : `${warnings.length} advertencias de política detectadas.`,
  )
}
