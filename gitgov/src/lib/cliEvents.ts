import type { TerminalLineType } from '@/lib/types'

export const CLI_LINE_EVENT = 'gitgov:cli-line'

export interface CliLinePayload {
  lineType: TerminalLineType
  text: string
}

export function emitCliLine(payload: CliLinePayload): void {
  if (typeof window === 'undefined') return
  window.dispatchEvent(new CustomEvent<CliLinePayload>(CLI_LINE_EVENT, { detail: payload }))
}

export function onCliLine(handler: (payload: CliLinePayload) => void): () => void {
  if (typeof window === 'undefined') {
    return () => {}
  }

  const listener: EventListener = (event) => {
    const customEvent = event as CustomEvent<CliLinePayload>
    if (!customEvent.detail) return
    handler(customEvent.detail)
  }

  window.addEventListener(CLI_LINE_EVENT, listener)
  return () => {
    window.removeEventListener(CLI_LINE_EVENT, listener)
  }
}

