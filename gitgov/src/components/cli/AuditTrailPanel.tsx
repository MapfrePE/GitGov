import { useCallback, useEffect, useRef, useState } from 'react'
import { tauriInvoke, tauriListen, parseCommandError } from '@/lib/tauri'
import { onCliLine } from '@/lib/cliEvents'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { useRepoStore } from '@/store/useRepoStore'
import { Activity, History, RefreshCw } from 'lucide-react'
import type { CliFinishedEvent, CliOutputEvent } from '@/lib/types'

type TrailMode = 'session' | 'history'
type TrailStatus = 'running' | 'success' | 'failed'
type TrailOrigin = 'button_click' | 'manual_input'

interface SessionTrailEntry {
  id: string
  command: string
  origin: TrailOrigin
  status: TrailStatus
  branch: string
  created_at: number
  message?: string
}

interface CliCommandRecord {
  id: string
  user_login: string
  command: string
  origin: TrailOrigin | string
  branch: string
  repo_name?: string | null
  exit_code?: number | null
  duration_ms?: number | null
  created_at: number
}

interface CliCommandListResponse {
  commands: CliCommandRecord[]
  total: number
}

const MAX_SESSION_ENTRIES = 120

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleTimeString([], {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

function statusBadge(status: TrailStatus | 'pending'): string {
  if (status === 'running') return 'bg-brand-500/15 text-brand-300 border-brand-500/30'
  if (status === 'success') return 'bg-success-500/15 text-success-300 border-success-500/30'
  if (status === 'failed') return 'bg-danger-500/15 text-danger-300 border-danger-500/30'
  return 'bg-surface-800 text-surface-400 border-surface-700'
}

export function AuditTrailPanel() {
  const serverConfig = useControlPlaneStore((s) => s.serverConfig)
  const currentBranch = useRepoStore((s) => s.currentBranch)

  const [mode, setMode] = useState<TrailMode>('session')
  const [sessionEntries, setSessionEntries] = useState<SessionTrailEntry[]>([])
  const [historyEntries, setHistoryEntries] = useState<CliCommandRecord[]>([])
  const [historyTotal, setHistoryTotal] = useState(0)
  const [isHistoryLoading, setIsHistoryLoading] = useState(false)
  const [historyError, setHistoryError] = useState<string | null>(null)

  const pendingByCommandIdRef = useRef<Map<string, string>>(new Map())
  const lastButtonPendingRef = useRef<string | null>(null)

  const appendSessionEntry = useCallback((entry: SessionTrailEntry) => {
    setSessionEntries((prev) => {
      const next = [entry, ...prev]
      return next.length > MAX_SESSION_ENTRIES ? next.slice(0, MAX_SESSION_ENTRIES) : next
    })
  }, [])

  const updateSessionEntry = useCallback((id: string, patch: Partial<SessionTrailEntry>) => {
    setSessionEntries((prev) =>
      prev.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)),
    )
  }, [])

  const loadHistory = useCallback(async () => {
    if (!serverConfig?.url) {
      setHistoryEntries([])
      setHistoryTotal(0)
      setHistoryError('Control Plane no configurado')
      return
    }

    setIsHistoryLoading(true)
    setHistoryError(null)
    try {
      const config = { url: serverConfig.url, api_key: serverConfig.api_key }
      const response = await tauriInvoke<CliCommandListResponse>('cmd_server_list_cli_commands', {
        config,
        user_login: null,
        limit: 80,
        offset: 0,
      })
      setHistoryEntries(response.commands ?? [])
      setHistoryTotal(response.total ?? 0)
    } catch (e) {
      setHistoryError(parseCommandError(String(e)).message)
    } finally {
      setIsHistoryLoading(false)
    }
  }, [serverConfig])

  useEffect(() => {
    if (mode === 'history') {
      void loadHistory()
    }
  }, [mode, loadHistory])

  useEffect(() => {
    return onCliLine(({ lineType, text }) => {
      if (lineType === 'command') {
        const command = text.startsWith('$ ') ? text.slice(2).trim() : text.trim()
        if (!command) return
        const id = `btn-${Date.now()}-${Math.random()}`
        lastButtonPendingRef.current = id
        appendSessionEntry({
          id,
          command,
          origin: 'button_click',
          status: 'running',
          branch: currentBranch ?? 'unknown',
          created_at: Date.now(),
        })
        return
      }

      const pendingId = lastButtonPendingRef.current
      if (!pendingId) return

      if (lineType === 'gitgov') {
        updateSessionEntry(pendingId, { status: 'success', message: text })
        lastButtonPendingRef.current = null
      } else if (lineType === 'stderr') {
        updateSessionEntry(pendingId, { status: 'failed', message: text })
        lastButtonPendingRef.current = null
      }
    })
  }, [appendSessionEntry, currentBranch, updateSessionEntry])

  useEffect(() => {
    let unlistenOutput: (() => void) | null = null
    let unlistenFinished: (() => void) | null = null

    const setup = async () => {
      unlistenOutput = await tauriListen<CliOutputEvent>('gitgov:cli-output', (payload) => {
        if (payload.line_type !== 'system' || !payload.text.startsWith('$ ')) return
        const command = payload.text.slice(2).trim()
        if (!command) return
        const id = `manual-${payload.command_id}`
        pendingByCommandIdRef.current.set(payload.command_id, id)
        appendSessionEntry({
          id,
          command,
          origin: 'manual_input',
          status: 'running',
          branch: currentBranch ?? 'unknown',
          created_at: Date.now(),
        })
      })

      unlistenFinished = await tauriListen<CliFinishedEvent>('gitgov:cli-finished', (payload) => {
        const id = pendingByCommandIdRef.current.get(payload.command_id)
        if (!id) return
        updateSessionEntry(id, {
          status: payload.exit_code === 0 ? 'success' : 'failed',
          message:
            payload.exit_code === 0
              ? 'Process completed successfully'
              : `Process exited with code ${payload.exit_code}`,
        })
        pendingByCommandIdRef.current.delete(payload.command_id)
      })
    }

    void setup()
    return () => {
      unlistenOutput?.()
      unlistenFinished?.()
    }
  }, [appendSessionEntry, currentBranch, updateSessionEntry])

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-surface-950">
      <div className="flex items-center justify-between border-b border-surface-800 bg-surface-900/70 px-3 py-1.5">
        <div className="flex items-center gap-2">
          <History size={12} className="text-surface-500" />
          <span className="text-[10px] font-medium uppercase tracking-wider text-surface-400">
            Audit Trail
          </span>
          {mode === 'history' && (
            <span className="rounded border border-surface-700 bg-surface-800 px-1.5 py-0.5 text-[8px] text-surface-400">
              {historyTotal} events
            </span>
          )}
        </div>

        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setMode('session')}
            className={`rounded border px-2 py-0.5 text-[9px] uppercase tracking-wider ${
              mode === 'session'
                ? 'border-brand-500/40 bg-brand-500/15 text-brand-300'
                : 'border-surface-700 bg-surface-800 text-surface-500'
            }`}
          >
            Session
          </button>
          <button
            type="button"
            onClick={() => setMode('history')}
            className={`rounded border px-2 py-0.5 text-[9px] uppercase tracking-wider ${
              mode === 'history'
                ? 'border-brand-500/40 bg-brand-500/15 text-brand-300'
                : 'border-surface-700 bg-surface-800 text-surface-500'
            }`}
          >
            History
          </button>
          {mode === 'history' && (
            <button
              type="button"
              onClick={() => void loadHistory()}
              className="rounded border border-surface-700 bg-surface-800 px-1.5 py-1 text-surface-400 hover:text-surface-200"
              aria-label="Refrescar historial"
              title="Refrescar"
            >
              <RefreshCw size={10} />
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-auto px-2 py-2">
        {mode === 'session' && sessionEntries.length === 0 && (
          <div className="flex h-full items-center justify-center text-surface-600">
            <div className="flex items-center gap-2 text-[10px] uppercase tracking-wider">
              <Activity size={11} />
              No session activity
            </div>
          </div>
        )}

        {mode === 'session' && sessionEntries.length > 0 && (
          <div className="space-y-1.5">
            {sessionEntries.map((entry) => (
              <div
                key={entry.id}
                className="rounded-md border border-surface-800 bg-surface-900/70 px-2 py-1.5"
              >
                <div className="flex items-center gap-1.5">
                  <span
                    className={`rounded border px-1.5 py-0.5 text-[8px] uppercase tracking-wider ${statusBadge(entry.status)}`}
                  >
                    {entry.status}
                  </span>
                  <span className="rounded border border-surface-700 bg-surface-800 px-1.5 py-0.5 text-[8px] uppercase tracking-wider text-surface-400">
                    {entry.origin === 'button_click' ? 'button' : 'manual'}
                  </span>
                  <span className="ml-auto text-[9px] text-surface-500">
                    {formatTimestamp(entry.created_at)}
                  </span>
                </div>
                <p className="mt-1 truncate font-mono text-[10px] text-surface-200">
                  {entry.command}
                </p>
                <p className="mt-0.5 text-[9px] text-surface-500">
                  {entry.branch}
                  {entry.message ? ` · ${entry.message}` : ''}
                </p>
              </div>
            ))}
          </div>
        )}

        {mode === 'history' && isHistoryLoading && (
          <div className="flex h-full items-center justify-center text-[10px] uppercase tracking-wider text-surface-500">
            Cargando historial...
          </div>
        )}

        {mode === 'history' && !isHistoryLoading && historyError && (
          <div className="flex h-full items-center justify-center px-4 text-center text-[10px] text-danger-400">
            {historyError}
          </div>
        )}

        {mode === 'history' && !isHistoryLoading && !historyError && historyEntries.length === 0 && (
          <div className="flex h-full items-center justify-center text-[10px] uppercase tracking-wider text-surface-600">
            No history data
          </div>
        )}

        {mode === 'history' && !isHistoryLoading && !historyError && historyEntries.length > 0 && (
          <div className="space-y-1.5">
            {historyEntries.map((entry) => {
              const entryStatus: TrailStatus | 'pending' =
                entry.exit_code == null
                  ? 'pending'
                  : entry.exit_code === 0
                    ? 'success'
                    : 'failed'

              return (
                <div
                  key={entry.id}
                  className="rounded-md border border-surface-800 bg-surface-900/70 px-2 py-1.5"
                >
                  <div className="flex items-center gap-1.5">
                    <span
                      className={`rounded border px-1.5 py-0.5 text-[8px] uppercase tracking-wider ${statusBadge(entryStatus)}`}
                    >
                      {entryStatus}
                    </span>
                    <span className="rounded border border-surface-700 bg-surface-800 px-1.5 py-0.5 text-[8px] uppercase tracking-wider text-surface-400">
                      {entry.origin === 'button_click' ? 'button' : 'manual'}
                    </span>
                    <span className="ml-auto text-[9px] text-surface-500">
                      {formatTimestamp(entry.created_at)}
                    </span>
                  </div>
                  <p className="mt-1 truncate font-mono text-[10px] text-surface-200">
                    {entry.command}
                  </p>
                  <p className="mt-0.5 text-[9px] text-surface-500">
                    {entry.branch}
                    {entry.duration_ms != null ? ` · ${entry.duration_ms}ms` : ''}
                  </p>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
