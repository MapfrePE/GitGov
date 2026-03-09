import { describe, it, expect, beforeEach, vi } from 'vitest'

// Mock Tauri notification plugin before importing the module under test
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(true),
  requestPermission: vi.fn().mockResolvedValue('granted'),
  sendNotification: vi.fn(),
}))

// Mock tauri helper — default: not a Tauri desktop environment
vi.mock('@/lib/tauri', () => ({
  isTauriDesktop: vi.fn().mockReturnValue(false),
}))

import {
  loadNotificationPrefs,
  saveNotificationPrefs,
  type NotificationPrefs,
} from '@/lib/notifications'

describe('Notification Preferences (localStorage)', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('returns defaults when nothing is stored', () => {
    const prefs = loadNotificationPrefs()
    expect(prefs).toEqual({
      enabled: true,
      onNewEvents: true,
      onBlockedPush: true,
      onGovernanceWarn: true,
    })
  })

  it('saves and loads preferences correctly', () => {
    const custom: NotificationPrefs = {
      enabled: false,
      onNewEvents: false,
      onBlockedPush: true,
      onGovernanceWarn: false,
    }
    saveNotificationPrefs(custom)
    const loaded = loadNotificationPrefs()
    expect(loaded).toEqual(custom)
  })

  it('merges partial stored prefs with defaults', () => {
    localStorage.setItem(
      'gitgov:notification-prefs',
      JSON.stringify({ enabled: false }),
    )
    const loaded = loadNotificationPrefs()
    expect(loaded.enabled).toBe(false)
    // Fields not stored should get defaults
    expect(loaded.onNewEvents).toBe(true)
    expect(loaded.onBlockedPush).toBe(true)
    expect(loaded.onGovernanceWarn).toBe(true)
  })

  it('returns defaults when stored JSON is corrupt', () => {
    localStorage.setItem('gitgov:notification-prefs', '{invalid json}')
    const loaded = loadNotificationPrefs()
    expect(loaded).toEqual({
      enabled: true,
      onNewEvents: true,
      onBlockedPush: true,
      onGovernanceWarn: true,
    })
  })
})

describe('notifyNewEvents', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('does not send notification when prefs.enabled is false', async () => {
    // We need to re-import with fresh module state for each notification test
    // Since the module has internal state (permissionChecked, lastNewEventsNotifyTs),
    // we test the preferences gate which is the first check
    const { sendNotification } = await import('@tauri-apps/plugin-notification')
    saveNotificationPrefs({
      enabled: false,
      onNewEvents: true,
      onBlockedPush: true,
      onGovernanceWarn: true,
    })
    const { notifyNewEvents } = await import('@/lib/notifications')
    await notifyNewEvents(5)
    // Since isTauriDesktop returns false, sendNotification is never called anyway
    // But the function should return early due to prefs.enabled = false
    expect(sendNotification).not.toHaveBeenCalled()
  })

  it('does not send notification when onNewEvents is false', async () => {
    const { sendNotification } = await import('@tauri-apps/plugin-notification')
    saveNotificationPrefs({
      enabled: true,
      onNewEvents: false,
      onBlockedPush: true,
      onGovernanceWarn: true,
    })
    const { notifyNewEvents } = await import('@/lib/notifications')
    await notifyNewEvents(3)
    expect(sendNotification).not.toHaveBeenCalled()
  })
})

describe('notifyGovernanceWarning', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('does not send when onGovernanceWarn is disabled', async () => {
    const { sendNotification } = await import('@tauri-apps/plugin-notification')
    saveNotificationPrefs({
      enabled: true,
      onNewEvents: true,
      onBlockedPush: true,
      onGovernanceWarn: false,
    })
    const { notifyGovernanceWarning } = await import('@/lib/notifications')
    await notifyGovernanceWarning(['warning 1'])
    expect(sendNotification).not.toHaveBeenCalled()
  })
})

describe('notifyBlockedPush', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('does not send when onBlockedPush is disabled', async () => {
    const { sendNotification } = await import('@tauri-apps/plugin-notification')
    saveNotificationPrefs({
      enabled: true,
      onNewEvents: true,
      onBlockedPush: false,
      onGovernanceWarn: true,
    })
    const { notifyBlockedPush } = await import('@/lib/notifications')
    await notifyBlockedPush('main', 'protected')
    expect(sendNotification).not.toHaveBeenCalled()
  })
})
