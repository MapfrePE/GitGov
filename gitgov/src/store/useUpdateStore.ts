import { create } from 'zustand'
import { toast } from '@/components/shared/Toast'
import {
  canUseDesktopUpdater,
  checkDesktopUpdate,
  downloadAndInstallDesktopUpdate,
  getDesktopUpdateFallbackUrl,
  isUpdaterNotConfiguredError,
  normalizeUpdaterErrorMessage,
  type DesktopUpdateChannel,
  type DesktopUpdateInfo,
  type DesktopUpdateProgress,
} from '@/lib/updater'

type UpdaterStatus =
  | 'idle'
  | 'checking'
  | 'no-update'
  | 'update-available'
  | 'downloading'
  | 'installed'
  | 'error'
  | 'unsupported'
  | 'not-configured'

interface UpdateStoreState {
  status: UpdaterStatus
  isChecking: boolean
  isDownloading: boolean
  isUpdaterSupported: boolean
  isUpdaterConfigured: boolean
  updateInfo: DesktopUpdateInfo | null
  progress: DesktopUpdateProgress | null
  lastCheckedAt: number | null
  error: string | null
  channel: DesktopUpdateChannel
  fallbackDownloadUrl: string
  changelogExpanded: boolean
  telemetry: {
    checks: number
    updateChecksWithUpdate: number
    downloadAttempts: number
    installSuccesses: number
    installFailures: number
    lastOutcome: 'none' | 'no-update' | 'update-available' | 'install-success' | 'install-failure' | 'check-error'
    lastError: string | null
    lastEventAt: number | null
  }
  _updateHandle: unknown | null
}

interface UpdateStoreActions {
  initializeUpdater: () => Promise<void>
  checkForUpdates: (opts?: { manual?: boolean; force?: boolean }) => Promise<void>
  downloadAndInstall: () => Promise<void>
  retryDownload: () => Promise<void>
  setChannel: (channel: DesktopUpdateChannel) => void
  clearError: () => void
  dismissUpdate: () => void
  setChangelogExpanded: (expanded: boolean) => void
}

const LAST_CHECK_KEY = 'gitgov:desktop-updater:last-check-at'
const CHANNEL_KEY = 'gitgov:desktop-updater:channel'
const TELEMETRY_KEY = 'gitgov:desktop-updater:telemetry'
const AUTO_CHECK_INTERVAL_MS = 1000 * 60 * 60 * 6

function readLastCheckAt(): number | null {
  try {
    const raw = window.localStorage.getItem(LAST_CHECK_KEY)
    if (!raw) return null
    const parsed = Number(raw)
    return Number.isFinite(parsed) ? parsed : null
  } catch {
    return null
  }
}

function writeLastCheckAt(value: number): void {
  try {
    window.localStorage.setItem(LAST_CHECK_KEY, String(value))
  } catch {
    // no-op
  }
}

function readChannel(): DesktopUpdateChannel {
  try {
    const raw = window.localStorage.getItem(CHANNEL_KEY)
    return raw === 'beta' ? 'beta' : 'stable'
  } catch {
    return 'stable'
  }
}

function writeChannel(value: DesktopUpdateChannel): void {
  try {
    window.localStorage.setItem(CHANNEL_KEY, value)
  } catch {
    // no-op
  }
}

type UpdateTelemetry = UpdateStoreState['telemetry']
type UpdateStoreShape = UpdateStoreState & UpdateStoreActions
type UpdateStoreSet = (
  partial:
    | Partial<UpdateStoreShape>
    | ((state: UpdateStoreShape) => Partial<UpdateStoreShape>)
) => void

function defaultTelemetry(): UpdateTelemetry {
  return {
    checks: 0,
    updateChecksWithUpdate: 0,
    downloadAttempts: 0,
    installSuccesses: 0,
    installFailures: 0,
    lastOutcome: 'none',
    lastError: null,
    lastEventAt: null,
  }
}

function readTelemetry(): UpdateTelemetry {
  try {
    const raw = window.localStorage.getItem(TELEMETRY_KEY)
    if (!raw) return defaultTelemetry()
    const parsed = JSON.parse(raw) as Partial<UpdateTelemetry>
    return {
      ...defaultTelemetry(),
      ...parsed,
      lastOutcome: parsed.lastOutcome ?? 'none',
    }
  } catch {
    return defaultTelemetry()
  }
}

function writeTelemetry(value: UpdateTelemetry): void {
  try {
    window.localStorage.setItem(TELEMETRY_KEY, JSON.stringify(value))
  } catch {
    // no-op
  }
}

function updateTelemetry(mutator: (current: UpdateTelemetry) => UpdateTelemetry, setState: UpdateStoreSet) {
  setState((state) => {
    const nextTelemetry = mutator(state.telemetry)
    writeTelemetry(nextTelemetry)
    return { telemetry: nextTelemetry }
  })
}

export const useUpdateStore = create<UpdateStoreState & UpdateStoreActions>((set, get) => ({
  status: canUseDesktopUpdater() ? 'idle' : 'unsupported',
  isChecking: false,
  isDownloading: false,
  isUpdaterSupported: canUseDesktopUpdater(),
  isUpdaterConfigured: true,
  updateInfo: null,
  progress: null,
  lastCheckedAt: null,
  error: null,
  channel: 'stable',
  fallbackDownloadUrl: getDesktopUpdateFallbackUrl('stable'),
  changelogExpanded: false,
  telemetry: defaultTelemetry(),
  _updateHandle: null,

  initializeUpdater: async () => {
    const supported = canUseDesktopUpdater()
    const lastCheckedAt = readLastCheckAt()
    const channel = readChannel()
    set({
      isUpdaterSupported: supported,
      channel,
      fallbackDownloadUrl: getDesktopUpdateFallbackUrl(channel),
      lastCheckedAt,
      telemetry: readTelemetry(),
      status: supported ? 'idle' : 'unsupported',
    })

    if (!supported) return
    await get().checkForUpdates({ manual: false, force: false })
  },

  checkForUpdates: async (opts) => {
    const manual = opts?.manual === true
    const force = opts?.force === true
    const supported = canUseDesktopUpdater()

    if (!supported) {
      set({
        status: 'unsupported',
        isUpdaterSupported: false,
        error: 'El updater está disponible solo en la app Desktop (Tauri).',
      })
      if (manual) {
        toast('warning', 'Las actualizaciones in-app solo funcionan en GitGov Desktop.')
      }
      return
    }

    const now = Date.now()
    const { lastCheckedAt, isChecking } = get()
    if (!manual && !force && isChecking) return
    if (!manual && !force && lastCheckedAt && now - lastCheckedAt < AUTO_CHECK_INTERVAL_MS) {
      return
    }

    set({
      isChecking: true,
      error: null,
      status: 'checking',
      isUpdaterConfigured: true,
      progress: null,
    })

    try {
      const channel = get().channel
      updateTelemetry((current) => ({
        ...current,
        checks: current.checks + 1,
        lastEventAt: now,
        lastError: null,
      }), set)
      const update = await checkDesktopUpdate(channel)
      const checkedAt = Date.now()
      writeLastCheckAt(checkedAt)

      if (!update) {
        set({
          isChecking: false,
          lastCheckedAt: checkedAt,
          status: 'no-update',
          updateInfo: null,
          _updateHandle: null,
        })
        if (manual) {
          toast('success', 'GitGov ya está actualizado.')
        }
        updateTelemetry((current) => ({
          ...current,
          lastOutcome: 'no-update',
          lastEventAt: checkedAt,
        }), set)
        return
      }

      const info: DesktopUpdateInfo = {
        currentVersion: update.currentVersion,
        version: update.version,
        date: update.date,
        body: update.body,
        rawJson: update.rawJson,
      }
      set({
        isChecking: false,
        lastCheckedAt: checkedAt,
        status: 'update-available',
        updateInfo: info,
        _updateHandle: update,
        changelogExpanded: false,
      })
      updateTelemetry((current) => ({
        ...current,
        updateChecksWithUpdate: current.updateChecksWithUpdate + 1,
        lastOutcome: 'update-available',
        lastEventAt: checkedAt,
      }), set)
      toast('info', `Nueva versión disponible: ${info.version}`)
    } catch (error) {
      const message = normalizeUpdaterErrorMessage(error)
      const notConfigured = isUpdaterNotConfiguredError(error)
      set({
        isChecking: false,
        error: message,
        isUpdaterConfigured: !notConfigured,
        status: notConfigured ? 'not-configured' : 'error',
      })
      if (manual) {
        toast(
          notConfigured ? 'warning' : 'error',
          notConfigured
            ? 'Updater no configurado aún (faltan endpoint/pubkey). Usa descarga manual.'
            : 'No se pudo verificar actualizaciones.'
        )
      }
      updateTelemetry((current) => ({
        ...current,
        lastOutcome: 'check-error',
        lastError: message,
        lastEventAt: Date.now(),
      }), set)
    }
  },

  downloadAndInstall: async () => {
    const { _updateHandle, updateInfo, isDownloading } = get()
    if (!_updateHandle || !updateInfo || isDownloading) return

    set({
      isDownloading: true,
      status: 'downloading',
      error: null,
      progress: { downloadedBytes: 0 },
    })
    updateTelemetry((current) => ({
      ...current,
      downloadAttempts: current.downloadAttempts + 1,
      lastError: null,
      lastEventAt: Date.now(),
    }), set)

    try {
      await downloadAndInstallDesktopUpdate(_updateHandle as never, (progress) => {
        set({ progress })
      }, { channel: get().channel })
      set({
        isDownloading: false,
        status: 'installed',
        progress: null,
      })
      updateTelemetry((current) => ({
        ...current,
        installSuccesses: current.installSuccesses + 1,
        lastOutcome: 'install-success',
        lastEventAt: Date.now(),
        lastError: null,
      }), set)
      toast('success', `Actualización ${updateInfo.version} instalada. Reinicia GitGov para aplicar cambios.`)
    } catch (error) {
      const message = normalizeUpdaterErrorMessage(error)
      set({
        isDownloading: false,
        status: 'error',
        error: message,
      })
      updateTelemetry((current) => ({
        ...current,
        installFailures: current.installFailures + 1,
        lastOutcome: 'install-failure',
        lastEventAt: Date.now(),
        lastError: message,
      }), set)
      toast('error', 'Falló la descarga/instalación del update. Usa descarga manual o reintenta.')
    }
  },

  retryDownload: async () => {
    await get().downloadAndInstall()
  },

  setChannel: (channel) => {
    writeChannel(channel)
    set({
      channel,
      fallbackDownloadUrl: getDesktopUpdateFallbackUrl(channel),
    })
  },

  clearError: () => set({ error: null }),

  dismissUpdate: () => set({
    updateInfo: null,
    _updateHandle: null,
    progress: null,
    status: 'idle',
  }),

  setChangelogExpanded: (expanded) => set({ changelogExpanded: expanded }),
}))
