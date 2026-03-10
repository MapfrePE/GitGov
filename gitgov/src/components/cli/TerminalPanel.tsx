import { useCallback, useEffect, useRef, useState } from 'react'
import { tauriInvoke, tauriListen } from '@/lib/tauri'
import { useRepoStore } from '@/store/useRepoStore'
import { useAuthStore } from '@/store/useAuthStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { onCliLine } from '@/lib/cliEvents'
import { Terminal } from 'lucide-react'
import { Terminal as XTerm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'

interface CliNativeTerminalStartResult {
  session_id: string
  shell: string
}

interface CliNativeTerminalOutputEvent {
  session_id: string
  data: string
}

interface CliNativeTerminalExitEvent {
  session_id: string
  exit_code: number
}

const ANSI = {
  reset: '\x1b[0m',
  muted: '\x1b[38;5;245m',
  success: '\x1b[38;5;83m',
  warning: '\x1b[38;5;220m',
  error: '\x1b[38;5;203m',
  command: '\x1b[38;5;111m',
}

export function TerminalPanel() {
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [shellName, setShellName] = useState('shell')
  const [isConnecting, setIsConnecting] = useState(false)

  const containerRef = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<XTerm | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const sessionIdRef = useRef<string | null>(null)
  const sessionCwdRef = useRef<string | null>(null)
  const writeQueueRef = useRef<Promise<void>>(Promise.resolve())

  const repoPath = useRepoStore((s) => s.repoPath)
  const currentBranch = useRepoStore((s) => s.currentBranch)
  const user = useAuthStore((s) => s.user)
  const serverConfig = useControlPlaneStore((s) => s.serverConfig)

  const writeSystem = useCallback((text: string, color: string = ANSI.muted) => {
    const terminal = terminalRef.current
    if (!terminal) return
    terminal.writeln(`${color}${text}${ANSI.reset}`)
  }, [])

  const stopNativeSession = useCallback(async (targetSessionId?: string | null) => {
    const resolved = targetSessionId ?? sessionIdRef.current
    if (!resolved) return
    try {
      await tauriInvoke('cmd_stop_native_terminal', { sessionId: resolved })
    } catch {
      // Best-effort stop.
    } finally {
      if (sessionIdRef.current === resolved) {
        sessionIdRef.current = null
        sessionCwdRef.current = null
        setSessionId(null)
      }
    }
  }, [])

  const sendResize = useCallback(async () => {
    const terminal = terminalRef.current
    const sid = sessionIdRef.current
    if (!terminal || !sid) return

    try {
      await tauriInvoke('cmd_resize_native_terminal', {
        request: {
          session_id: sid,
          cols: terminal.cols,
          rows: terminal.rows,
        },
      })
    } catch {
      // Ignore transient resize failures.
    }
  }, [])

  const startNativeSession = useCallback(
    async (forceRestart = false) => {
      const terminal = terminalRef.current
      const fitAddon = fitAddonRef.current

      if (!terminal || !fitAddon) return

      if (!repoPath) {
        await stopNativeSession()
        terminal.clear()
        writeSystem('Select a repository to start native terminal session.')
        return
      }

      if (!forceRestart && sessionIdRef.current && sessionCwdRef.current === repoPath) {
        return
      }

      if (sessionIdRef.current) {
        await stopNativeSession(sessionIdRef.current)
      }

      setIsConnecting(true)
      fitAddon.fit()

      try {
        const result = await tauriInvoke<CliNativeTerminalStartResult>('cmd_start_native_terminal', {
          request: {
            cwd: repoPath,
            cols: terminal.cols,
            rows: terminal.rows,
          },
        })

        sessionIdRef.current = result.session_id
        sessionCwdRef.current = repoPath
        setSessionId(result.session_id)
        setShellName(result.shell || 'shell')

        terminal.focus()
        writeSystem(`[GitGov] Native terminal connected (${result.shell})`, ANSI.success)
        await sendResize()
      } catch (e) {
        writeSystem(`Failed to start native terminal: ${String(e)}`, ANSI.error)
      } finally {
        setIsConnecting(false)
      }
    },
    [repoPath, sendResize, stopNativeSession, writeSystem],
  )

  useEffect(() => {
    const terminal = new XTerm({
      fontFamily: 'Geist Mono, JetBrains Mono, Consolas, monospace',
      fontSize: 12,
      lineHeight: 1.2,
      cursorBlink: true,
      convertEol: false,
      scrollback: 5000,
      theme: {
        background: '#0a0b0e',
        foreground: '#d7dce4',
        cursor: '#f97316',
        cursorAccent: '#0a0b0e',
        selectionBackground: 'rgba(249,115,22,0.22)',
        black: '#0a0b0e',
        red: '#ef4444',
        green: '#22c55e',
        yellow: '#f59e0b',
        blue: '#60a5fa',
        magenta: '#a78bfa',
        cyan: '#22d3ee',
        white: '#e5e7eb',
        brightBlack: '#6b7280',
        brightRed: '#f87171',
        brightGreen: '#4ade80',
        brightYellow: '#fbbf24',
        brightBlue: '#93c5fd',
        brightMagenta: '#c4b5fd',
        brightCyan: '#67e8f9',
        brightWhite: '#f8fafc',
      },
    })
    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)

    if (containerRef.current) {
      terminal.open(containerRef.current)
      fitAddon.fit()
      terminal.focus()
    }

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    writeSystem('GitGov native terminal ready.')

    const dataDisposable = terminal.onData((data) => {
      const sid = sessionIdRef.current
      if (!sid) return

      writeQueueRef.current = writeQueueRef.current
        .then(async () => {
          await tauriInvoke('cmd_write_native_terminal', {
            request: { session_id: sid, data },
          })
        })
        .catch(() => {
          // Keep terminal responsive even if a write fails.
        })
    })

    return () => {
      dataDisposable.dispose()
      resizeObserverRef.current?.disconnect()
      resizeObserverRef.current = null
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [writeSystem])

  useEffect(() => {
    const container = containerRef.current
    const fitAddon = fitAddonRef.current
    if (!container || !fitAddon) return

    const observer = new ResizeObserver(() => {
      fitAddon.fit()
      void sendResize()
    })
    observer.observe(container)
    resizeObserverRef.current = observer

    return () => {
      observer.disconnect()
      if (resizeObserverRef.current === observer) {
        resizeObserverRef.current = null
      }
    }
  }, [sendResize])

  useEffect(() => {
    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null

    const setup = async () => {
      unlistenOutput = await tauriListen<CliNativeTerminalOutputEvent>('gitgov:pty-output', (event) => {
        if (event.session_id !== sessionIdRef.current) return
        terminalRef.current?.write(event.data)
      })

      unlistenExit = await tauriListen<CliNativeTerminalExitEvent>('gitgov:pty-exit', (event) => {
        if (event.session_id !== sessionIdRef.current) return
        writeSystem(`[GitGov] Shell exited with code ${event.exit_code}`, ANSI.warning)
        sessionIdRef.current = null
        sessionCwdRef.current = null
        setSessionId(null)
      })
    }

    void setup()
    return () => {
      unlistenOutput?.()
      unlistenExit?.()
    }
  }, [writeSystem])

  useEffect(() => {
    return onCliLine(({ lineType, text }) => {
      if (lineType === 'command') {
        writeSystem(text, ANSI.command)
        return
      }
      if (lineType === 'gitgov') {
        writeSystem(text, ANSI.success)
        return
      }
      if (lineType === 'stderr') {
        writeSystem(text, ANSI.error)
        return
      }
      writeSystem(text)
    })
  }, [writeSystem])

  useEffect(() => {
    void startNativeSession()
  }, [startNativeSession, repoPath, user?.login, currentBranch, serverConfig])

  useEffect(() => {
    return () => {
      void stopNativeSession()
    }
  }, [stopNativeSession])

  return (
    <div className="flex h-full min-h-0 flex-col bg-surface-950">
      <div className="flex items-center gap-2 border-b border-surface-800 bg-surface-900/60 px-3 py-1.5">
        <Terminal size={12} className="text-surface-500" />
        <span className="text-[10px] font-medium uppercase tracking-wider text-surface-400">
          Terminal
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            onClick={() => void startNativeSession(true)}
            className="rounded border border-surface-700 bg-surface-900 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-surface-400 transition-colors hover:text-surface-200"
            title="Reconnect native terminal session"
          >
            Reconnect
          </button>
          <span
            className={`rounded border px-1.5 py-0.5 text-[9px] ${
              sessionId && !isConnecting
                ? 'border-success-500/30 bg-success-500/10 text-success-300'
                : 'border-warning-500/30 bg-warning-500/10 text-warning-300'
            }`}
          >
            {sessionId && !isConnecting ? shellName : 'connecting...'}
          </span>
        </div>
      </div>

      <div className="flex-1 min-h-0 p-1.5">
        <div
          ref={containerRef}
          className="h-full w-full rounded-sm border border-surface-900 bg-surface-950 p-1"
        />
      </div>
    </div>
  )
}
