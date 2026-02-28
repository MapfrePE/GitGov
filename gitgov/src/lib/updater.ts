import { isTauriDesktop } from '@/lib/tauri'

export type DesktopUpdateChannel = 'stable' | 'beta'

export interface DesktopUpdateInfo {
  currentVersion: string
  version: string
  date?: string
  body?: string
  rawJson?: Record<string, unknown>
}

export interface DesktopUpdateProgress {
  downloadedBytes: number
  totalBytes?: number
}

const DEFAULT_FALLBACK_DOWNLOAD_URL = 'https://github.com/MapfrePE/GitGov/releases/latest'
const UPDATE_CHANNEL_HEADER = 'x-gitgov-update-channel'

function normalizeChannel(channel: string | undefined): DesktopUpdateChannel {
  return channel === 'beta' ? 'beta' : 'stable'
}

export function getDesktopUpdateFallbackUrl(channel?: DesktopUpdateChannel): string {
  const envValue = (import.meta.env.VITE_DESKTOP_DOWNLOAD_FALLBACK_URL as string | undefined)?.trim()
  const selectedChannel = normalizeChannel(channel)
  const base = envValue || DEFAULT_FALLBACK_DOWNLOAD_URL
  if (base.includes('{channel}')) {
    return base.replaceAll('{channel}', selectedChannel)
  }
  if (/\/releases\/latest$/i.test(base) || /\.exe$/i.test(base) || /\.json$/i.test(base)) {
    return base
  }
  return `${base.replace(/\/+$/, '')}/${selectedChannel}`
}

export function canUseDesktopUpdater(): boolean {
  return isTauriDesktop()
}

export function isUpdaterNotConfiguredError(error: unknown): boolean {
  const message = String(error ?? '').toLowerCase()
  return (
    message.includes('updater') &&
    (message.includes('config') ||
      message.includes('endpoint') ||
      message.includes('pubkey') ||
      message.includes('not configured'))
  )
}

function buildChannelHeaders(channel: DesktopUpdateChannel) {
  return {
    [UPDATE_CHANNEL_HEADER]: normalizeChannel(channel),
  }
}

export async function checkDesktopUpdate(channel: DesktopUpdateChannel = 'stable') {
  if (!canUseDesktopUpdater()) {
    throw new Error('Updater disponible solo en GitGov Desktop (Tauri).')
  }
  const updater = await import('@tauri-apps/plugin-updater')
  return updater.check({
    headers: buildChannelHeaders(channel),
  })
}

export async function downloadAndInstallDesktopUpdate(
  update: Awaited<ReturnType<typeof checkDesktopUpdate>> extends infer T
    ? T extends { downloadAndInstall: unknown }
      ? T
      : never
    : never,
  onProgress: (progress: DesktopUpdateProgress) => void,
  options?: { channel?: DesktopUpdateChannel }
) {
  let downloadedBytes = 0
  const channel = normalizeChannel(options?.channel)
  await update.downloadAndInstall((event) => {
    if (event.event === 'Started') {
      downloadedBytes = 0
      onProgress({ downloadedBytes, totalBytes: event.data.contentLength })
      return
    }
    if (event.event === 'Progress') {
      downloadedBytes += event.data.chunkLength
      onProgress({ downloadedBytes })
      return
    }
    if (event.event === 'Finished') {
      onProgress({ downloadedBytes })
    }
  }, {
    headers: buildChannelHeaders(channel),
  })
}
