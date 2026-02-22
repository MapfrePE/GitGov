import { invoke } from '@tauri-apps/api/core'

export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args)
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
