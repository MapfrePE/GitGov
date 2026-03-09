import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const isTauri = typeof window !== 'undefined' && '__TAURI__' in window

export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    return invoke<T>(cmd, args)
  }
  
  // When running in browser (not Tauri desktop), throw error
  // GitHub auth only works in Tauri desktop, not in browser
  throw new Error('Esta función requiere la aplicación desktop de GitGov. Descarga e instala GitGov Desktop para usar todas las funciones.')
}

export function parseCommandError(error: string): { code: string; message: string } {
  try {
    const parsed = JSON.parse(error)
    return {
      code: parsed.code || 'UNKNOWN',
      message: parsed.message || error,
    }
  } catch {
    return {
      code: 'UNKNOWN',
      message: error,
    }
  }
}

export function isTauriDesktop(): boolean {
  return isTauri
}

/** Subscribe to a Tauri backend event. Returns an unlisten function. */
export async function tauriListen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!isTauri) {
    return () => {}
  }
  return listen<T>(event, (e) => handler(e.payload))
}
