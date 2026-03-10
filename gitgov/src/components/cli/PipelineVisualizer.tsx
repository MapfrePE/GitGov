import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { tauriInvoke, tauriListen } from '@/lib/tauri'
import { onCliLine } from '@/lib/cliEvents'
import { useRepoStore } from '@/store/useRepoStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import type { CliFinishedEvent, CliOutputEvent, PipelineNodeStatus } from '@/lib/types'
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  Files,
  GitBranch,
  GitCommit,
  GitPullRequest,
  Loader2,
  Play,
  ShieldCheck,
  Ticket,
} from 'lucide-react'

const REFRESH_INTERVAL_MS = 30_000
const LIVE_TTL_MS = 10 * 60 * 1000
const TICKET_ID_REGEX = /\b([A-Z][A-Z0-9]{1,9}-\d+)\b/g

type FlowStepKey = 'ticket' | 'branch' | 'stage' | 'commit' | 'push' | 'pipeline'

interface CommitData {
  sha: string
  short_sha: string
  message?: string
  summary: string
  author: string
  timestamp: number
  branch: string
}

interface PipelineGraphData {
  current_branch: string
  target_branches: string[]
  commits: CommitData[]
}

interface PipelineRun {
  status: string
  job_name: string
  pipeline_id: string
}

interface PrEvidence {
  pr_number: number
  approvals_count: number
  pr_title?: string | null
}

interface FlowStepState {
  status: PipelineNodeStatus
  detail: string
  timestamp: number
}

interface LiveFeedEntry {
  id: string
  status: PipelineNodeStatus
  text: string
  timestamp: number
}

interface FlowStepDefinition {
  key: FlowStepKey
  label: string
  Icon: typeof Ticket
}

interface DeckCardMetric {
  label: string
  value: string
  status: PipelineNodeStatus
}

interface GateStatusItem {
  label: string
  detail: string
  status: PipelineNodeStatus
}

interface CurrentFocusState {
  label: string
  detail: string
  status: PipelineNodeStatus
  Icon: typeof Ticket
  nextAction: string
}

const FLOW_STEPS: FlowStepDefinition[] = [
  { key: 'ticket', label: 'Ticket', Icon: Ticket },
  { key: 'branch', label: 'Branch', Icon: GitBranch },
  { key: 'stage', label: 'Stage', Icon: Files },
  { key: 'commit', label: 'Commit', Icon: GitCommit },
  { key: 'push', label: 'Push / PR', Icon: GitPullRequest },
  { key: 'pipeline', label: 'CI Pipeline', Icon: Play },
]

const STATUS_STYLE: Record<
  PipelineNodeStatus,
  { card: string; icon: string; dot: string; connector: string }
> = {
  pending: {
    card: 'border-surface-700 bg-surface-900/80 text-surface-500',
    icon: 'text-surface-500',
    dot: 'bg-surface-600',
    connector: 'bg-surface-700',
  },
  active: {
    card: 'border-brand-500/50 bg-brand-500/10 text-brand-300 shadow-[0_0_10px_rgba(59,130,246,0.35)]',
    icon: 'text-brand-300',
    dot: 'bg-brand-400',
    connector: 'bg-brand-500/40',
  },
  success: {
    card: 'border-success-500/45 bg-success-500/10 text-success-300',
    icon: 'text-success-300',
    dot: 'bg-success-400',
    connector: 'bg-success-500/35',
  },
  warning: {
    card: 'border-warning-500/45 bg-warning-500/10 text-warning-300',
    icon: 'text-warning-300',
    dot: 'bg-warning-400',
    connector: 'bg-warning-500/35',
  },
  failed: {
    card: 'border-danger-500/50 bg-danger-500/10 text-danger-300',
    icon: 'text-danger-300',
    dot: 'bg-danger-400',
    connector: 'bg-danger-500/40',
  },
}

function extractTicketIds(text: string): string[] {
  const unique: string[] = []
  const seen = new Set<string>()
  for (const match of text.matchAll(TICKET_ID_REGEX)) {
    const ticket = String(match[1] ?? '').toUpperCase()
    if (!ticket || seen.has(ticket)) continue
    seen.add(ticket)
    unique.push(ticket)
  }
  return unique
}

function normalizePipelineStatus(status: string): PipelineNodeStatus {
  const n = status.trim().toLowerCase()
  if (n === 'success' || n === 'passed') return 'success'
  if (n === 'failure' || n === 'failed' || n === 'error') return 'failed'
  if (n === 'running' || n === 'in_progress' || n === 'building') return 'active'
  if (n === 'unstable' || n === 'aborted') return 'warning'
  return 'pending'
}

function detectStepFromCommand(command: string): FlowStepKey | null {
  const normalized = command.trim().toLowerCase()
  if (!normalized) return null
  if (normalized.startsWith('git add') || normalized.includes('restore --staged')) return 'stage'
  if (normalized.startsWith('git commit')) return 'commit'
  if (normalized.startsWith('git push')) return 'push'
  if (normalized.startsWith('git checkout') || normalized.startsWith('git switch') || normalized.startsWith('git branch')) return 'branch'
  return null
}

function inferSuccessStepFromText(text: string): FlowStepKey | null {
  const normalized = text.toLowerCase()
  if (normalized.includes('commit auditado')) return 'commit'
  if (normalized.includes('push auditado')) return 'push'
  if (normalized.includes('staging')) return 'stage'
  return null
}

function findBySha<T>(map: Map<string, T>, sha: string): T | null {
  const normalized = sha.toLowerCase()
  const exact = map.get(normalized)
  if (exact) return exact
  for (const [fullSha, value] of map.entries()) {
    if (fullSha.startsWith(normalized) || normalized.startsWith(fullSha.slice(0, 7))) {
      return value
    }
  }
  return null
}

function StatusGlyph({ status }: { status: PipelineNodeStatus }) {
  if (status === 'success') return <CheckCircle2 size={11} />
  if (status === 'active') return <Loader2 size={11} className="animate-spin" />
  if (status === 'warning' || status === 'failed') return <AlertTriangle size={11} />
  return <CircleDashed size={11} />
}

export function PipelineVisualizer() {
  const [graphData, setGraphData] = useState<PipelineGraphData | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [liveStepState, setLiveStepState] = useState<Partial<Record<FlowStepKey, FlowStepState>>>({})
  const [liveFeed, setLiveFeed] = useState<LiveFeedEntry[]>([])

  const repoPath = useRepoStore((s) => s.repoPath)
  const currentBranch = useRepoStore((s) => s.currentBranch)
  const stagedFiles = useRepoStore((s) => s.stagedFiles)

  const serverConfig = useControlPlaneStore((s) => s.serverConfig)
  const sseConnected = useControlPlaneStore((s) => s.sseConnected)
  const jenkinsCorrelations = useControlPlaneStore((s) => s.jenkinsCorrelations)
  const prMergeEvidence = useControlPlaneStore((s) => s.prMergeEvidence)
  const jiraTicketDetails = useControlPlaneStore((s) => s.jiraTicketDetails)
  const loadJenkinsCorrelations = useControlPlaneStore((s) => s.loadJenkinsCorrelations)
  const loadPrMergeEvidence = useControlPlaneStore((s) => s.loadPrMergeEvidence)
  const loadJiraTicketDetail = useControlPlaneStore((s) => s.loadJiraTicketDetail)

  const pendingStepByCommandIdRef = useRef<Map<string, FlowStepKey>>(new Map())
  const lastUiStepRef = useRef<FlowStepKey | null>(null)

  const pushFeed = useCallback((status: PipelineNodeStatus, text: string) => {
    const entry: LiveFeedEntry = {
      id: `${Date.now()}-${Math.random()}`,
      status,
      text,
      timestamp: Date.now(),
    }
    setLiveFeed((prev) => [entry, ...prev].slice(0, 8))
  }, [])

  const applyLiveState = useCallback(
    (step: FlowStepKey, status: PipelineNodeStatus, detail: string) => {
      setLiveStepState((prev) => ({
        ...prev,
        [step]: { status, detail, timestamp: Date.now() },
      }))
      pushFeed(status, `${step.toUpperCase()} · ${detail}`)
    },
    [pushFeed],
  )

  const loadGraph = useCallback(async () => {
    if (!repoPath) return
    try {
      const data = await tauriInvoke<PipelineGraphData>('cmd_get_pipeline_graph', {
        repoPath,
        maxCommits: 30,
      })
      setGraphData(data)
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [repoPath])

  const refreshControlPlaneSignals = useCallback(async () => {
    if (!serverConfig?.url || !serverConfig.api_key) return
    try {
      await Promise.all([loadJenkinsCorrelations(100), loadPrMergeEvidence(100)])
    } catch {
      // Non-fatal for visual panel.
    }
  }, [serverConfig, loadJenkinsCorrelations, loadPrMergeEvidence])

  useEffect(() => {
    void loadGraph()
    void refreshControlPlaneSignals()
  }, [loadGraph, refreshControlPlaneSignals, currentBranch])

  useEffect(() => {
    const interval = setInterval(() => {
      void loadGraph()
      void refreshControlPlaneSignals()
    }, REFRESH_INTERVAL_MS)
    return () => clearInterval(interval)
  }, [loadGraph, refreshControlPlaneSignals])

  useEffect(() => {
    let unlisten: (() => void) | null = null
    const setup = async () => {
      unlisten = await tauriListen<{ type?: string }>('gitgov:sse-event', (payload) => {
        const eventType = payload?.type
        if (eventType === 'new_events' || eventType === 'stats_updated') {
          void loadGraph()
          void refreshControlPlaneSignals()
          pushFeed('active', `SSE ${eventType}`)
        }
      })
    }
    void setup()
    return () => {
      unlisten?.()
    }
  }, [loadGraph, pushFeed, refreshControlPlaneSignals])

  useEffect(() => {
    if (!graphData || !serverConfig?.url || !serverConfig.api_key) return
    const ticketSet = new Set<string>()
    for (const commit of graphData.commits) {
      const text = `${commit.summary ?? ''}\n${commit.message ?? ''}`
      for (const ticket of extractTicketIds(text)) {
        ticketSet.add(ticket)
      }
    }
    for (const ticket of Array.from(ticketSet).slice(0, 8)) {
      void loadJiraTicketDetail(ticket)
    }
  }, [graphData, serverConfig, loadJiraTicketDetail])

  useEffect(() => {
    return onCliLine(({ lineType, text }) => {
      if (lineType === 'command') {
        const command = text.startsWith('$ ') ? text.slice(2).trim() : text.trim()
        const step = detectStepFromCommand(command)
        if (!step) return
        lastUiStepRef.current = step
        applyLiveState(step, 'active', command)
        return
      }

      if (lineType === 'gitgov') {
        const successStep = inferSuccessStepFromText(text) ?? lastUiStepRef.current
        if (!successStep) return
        applyLiveState(successStep, 'success', text)
        return
      }

      if (lineType === 'stderr') {
        const failedStep = lastUiStepRef.current
        if (!failedStep) return
        applyLiveState(failedStep, 'failed', text)
      }
    })
  }, [applyLiveState])

  useEffect(() => {
    let unlistenOutput: (() => void) | null = null
    let unlistenFinished: (() => void) | null = null

    const setup = async () => {
      unlistenOutput = await tauriListen<CliOutputEvent>('gitgov:cli-output', (payload) => {
        if (payload.line_type !== 'system' || !payload.text.startsWith('$ ')) return
        const command = payload.text.slice(2).trim()
        const step = detectStepFromCommand(command)
        if (!step) return
        pendingStepByCommandIdRef.current.set(payload.command_id, step)
        applyLiveState(step, 'active', command)
      })

      unlistenFinished = await tauriListen<CliFinishedEvent>('gitgov:cli-finished', (payload) => {
        const step = pendingStepByCommandIdRef.current.get(payload.command_id)
        if (!step) return
        applyLiveState(
          step,
          payload.exit_code === 0 ? 'success' : 'failed',
          payload.exit_code === 0
            ? 'Command completed successfully'
            : `Command failed (${payload.exit_code})`,
        )
        pendingStepByCommandIdRef.current.delete(payload.command_id)
      })
    }

    void setup()
    return () => {
      unlistenOutput?.()
      unlistenFinished?.()
    }
  }, [applyLiveState])

  useEffect(() => {
    const interval = setInterval(() => {
      const cutoff = Date.now() - LIVE_TTL_MS
      setLiveStepState((prev) => {
        const next: Partial<Record<FlowStepKey, FlowStepState>> = {}
        for (const key of Object.keys(prev) as FlowStepKey[]) {
          const entry = prev[key]
          if (entry && entry.timestamp >= cutoff) {
            next[key] = entry
          }
        }
        return next
      })
      setLiveFeed((prev) => prev.filter((entry) => entry.timestamp >= cutoff))
    }, 30_000)
    return () => clearInterval(interval)
  }, [])

  const pipelineBySha = useMemo(() => {
    const map = new Map<string, PipelineRun>()
    for (const item of jenkinsCorrelations) {
      const sha = String(item.commit_sha ?? '').trim().toLowerCase()
      if (!sha || !item.pipeline) continue
      if (!map.has(sha)) {
        map.set(sha, {
          status: item.pipeline.status,
          job_name: item.pipeline.job_name,
          pipeline_id: item.pipeline.pipeline_id,
        })
      }
    }
    return map
  }, [jenkinsCorrelations])

  const prBySha = useMemo(() => {
    const map = new Map<string, PrEvidence>()
    for (const item of prMergeEvidence) {
      const sha = String(item.head_sha ?? '').trim().toLowerCase()
      if (!sha) continue
      if (!map.has(sha)) {
        map.set(sha, {
          pr_number: item.pr_number,
          approvals_count: item.approvals_count,
          pr_title: item.pr_title,
        })
      }
    }
    return map
  }, [prMergeEvidence])

  const latestCommit = useMemo(() => graphData?.commits?.[0] ?? null, [graphData])

  const primaryTicketId = useMemo(() => {
    if (!latestCommit) return null
    const tickets = extractTicketIds(`${latestCommit.summary ?? ''}\n${latestCommit.message ?? ''}`)
    return tickets[0] ?? null
  }, [latestCommit])

  const primaryTicket = primaryTicketId ? (jiraTicketDetails[primaryTicketId] ?? null) : null
  const latestPrEvidence = latestCommit ? findBySha(prBySha, latestCommit.sha) : null
  const latestPipelineRun = latestCommit ? findBySha(pipelineBySha, latestCommit.sha) : null

  const steps = useMemo(() => {
    const now = Date.now()
    const base: Record<FlowStepKey, FlowStepState> = {
      ticket: { status: 'pending', detail: 'No ticket detected', timestamp: 0 },
      branch: {
        status: currentBranch ? 'success' : 'pending',
        detail: currentBranch ?? 'detached',
        timestamp: 0,
      },
      stage: {
        status: stagedFiles.size > 0 ? 'active' : 'pending',
        detail: stagedFiles.size > 0 ? `${stagedFiles.size} file(s) staged` : 'No staged files',
        timestamp: 0,
      },
      commit: { status: 'pending', detail: 'No commit in context', timestamp: 0 },
      push: { status: 'pending', detail: 'Awaiting push / PR', timestamp: 0 },
      pipeline: { status: 'pending', detail: 'Awaiting CI signal', timestamp: 0 },
    }

    if (latestCommit) {
      const commitTs = latestCommit.timestamp * 1000
      base.commit = {
        status: 'success',
        detail: `${latestCommit.short_sha} · ${latestCommit.summary || '(no message)'}`,
        timestamp: commitTs,
      }

      if (primaryTicketId) {
        const ticketDetail = primaryTicket
        const ticketStatus = String(ticketDetail?.status ?? '').toLowerCase()
        const status: PipelineNodeStatus =
          ticketStatus.includes('done') || ticketStatus.includes('closed') || ticketStatus.includes('resolved')
            ? 'success'
            : ticketStatus
              ? 'active'
              : 'pending'
        base.ticket = {
          status,
          detail: ticketDetail?.title ? `${primaryTicketId} · ${ticketDetail.title}` : primaryTicketId,
          timestamp: commitTs,
        }
      }

      if (latestPrEvidence) {
        base.push = {
          status: latestPrEvidence.approvals_count >= 2 ? 'success' : 'warning',
          detail: `PR #${latestPrEvidence.pr_number} · ${latestPrEvidence.approvals_count} approval(s)`,
          timestamp: commitTs,
        }
      } else {
        base.push = {
          status: 'pending',
          detail: `Commit ${latestCommit.short_sha} awaiting push/PR`,
          timestamp: commitTs,
        }
      }

      if (latestPipelineRun) {
        base.pipeline = {
          status: normalizePipelineStatus(latestPipelineRun.status),
          detail: `${latestPipelineRun.job_name || latestPipelineRun.pipeline_id} · ${latestPipelineRun.status}`,
          timestamp: commitTs,
        }
      }
    }

    for (const key of Object.keys(liveStepState) as FlowStepKey[]) {
      const live = liveStepState[key]
      if (!live) continue
      // While the live state is fresh, it should dominate to keep UI real-time.
      if (now - live.timestamp <= LIVE_TTL_MS) {
        base[key] = live
      }
    }

    return FLOW_STEPS.map((step) => ({
      ...step,
      ...base[step.key],
    }))
  }, [currentBranch, liveStepState, stagedFiles.size, latestCommit, latestPipelineRun, latestPrEvidence, primaryTicket, primaryTicketId])

  const currentFocus = useMemo<CurrentFocusState>(() => {
    const activeStep = steps.find((step) => step.status === 'active')
    if (activeStep) {
      return {
        label: activeStep.label,
        detail: activeStep.detail,
        status: activeStep.status,
        Icon: activeStep.Icon,
        nextAction:
          activeStep.key === 'stage'
            ? 'Create a commit from the staged changes.'
            : activeStep.key === 'commit'
              ? 'Push this branch or open a PR next.'
              : activeStep.key === 'push'
                ? 'Wait for PR evidence or CI feedback.'
                : 'Let the current operation finish before the next step.',
      }
    }

    const failedStep = steps.find((step) => step.status === 'failed')
    if (failedStep) {
      return {
        label: failedStep.label,
        detail: failedStep.detail,
        status: failedStep.status,
        Icon: failedStep.Icon,
        nextAction: 'Inspect the failure in the terminal or audit trail before continuing.',
      }
    }

    if (stagedFiles.size > 0) {
      return {
        label: 'Stage',
        detail: `${stagedFiles.size} file(s) are ready to commit.`,
        status: 'active' as PipelineNodeStatus,
        Icon: Files,
        nextAction: 'Create a commit to move this session forward.',
      }
    }

    if (latestPipelineRun) {
      return {
        label: 'CI Pipeline',
        detail: `${latestPipelineRun.job_name || latestPipelineRun.pipeline_id} · ${latestPipelineRun.status}`,
        status: normalizePipelineStatus(latestPipelineRun.status),
        Icon: Play,
        nextAction:
          normalizePipelineStatus(latestPipelineRun.status) === 'failed'
            ? 'Review CI output and correct the failing step.'
            : 'Monitor CI and merge only after the gate is green.',
      }
    }

    if (latestPrEvidence) {
      return {
        label: 'Push / PR',
        detail: `PR #${latestPrEvidence.pr_number} · ${latestPrEvidence.approvals_count} approval(s)`,
        status: latestPrEvidence.approvals_count >= 2 ? 'success' : 'warning',
        Icon: GitPullRequest,
        nextAction:
          latestPrEvidence.approvals_count >= 2
            ? 'CI is the remaining gate before merge.'
            : 'Collect approvals to clear review governance.',
      }
    }

    if (latestCommit) {
      return {
        label: 'Commit',
        detail: `${latestCommit.short_sha} · ${latestCommit.summary || '(no message)'}`,
        status: 'success' as PipelineNodeStatus,
        Icon: GitCommit,
        nextAction: 'Push this commit to generate PR and CI evidence.',
      }
    }

    return {
      label: 'Session',
      detail: currentBranch ? `Branch ${currentBranch} is ready.` : 'Select a working branch to begin.',
      status: currentBranch ? ('pending' as PipelineNodeStatus) : ('warning' as PipelineNodeStatus),
      Icon: Activity,
      nextAction: currentBranch
        ? 'Stage or edit files to start the flow.'
        : 'Create or checkout a branch before making changes.',
    }
  }, [currentBranch, latestCommit, latestPipelineRun, latestPrEvidence, stagedFiles.size, steps])

  const snapshotMetrics = useMemo<DeckCardMetric[]>(() => [
    {
      label: 'Ticket',
      value: primaryTicketId ?? 'Not linked',
      status: primaryTicketId ? steps.find((step) => step.key === 'ticket')?.status ?? 'pending' : 'pending',
    },
    {
      label: 'Branch',
      value: currentBranch ?? 'Detached',
      status: currentBranch ? 'success' : 'warning',
    },
    {
      label: 'Commit',
      value: latestCommit ? latestCommit.short_sha : 'No commit',
      status: latestCommit ? 'success' : 'pending',
    },
    {
      label: 'Stage',
      value: stagedFiles.size > 0 ? `${stagedFiles.size} staged` : 'Empty',
      status: stagedFiles.size > 0 ? 'active' : 'pending',
    },
  ], [currentBranch, latestCommit, primaryTicketId, stagedFiles.size, steps])

  const gateItems = useMemo<GateStatusItem[]>(() => {
    const ticketStep = steps.find((step) => step.key === 'ticket')
    const pushStep = steps.find((step) => step.key === 'push')
    const pipelineStep = steps.find((step) => step.key === 'pipeline')

    return [
      {
        label: 'Traceability',
        detail: primaryTicketId
          ? primaryTicket?.title
            ? `${primaryTicketId} · ${primaryTicket.title}`
            : primaryTicketId
          : 'No ticket detected in branch or commit message',
        status: ticketStep?.status ?? 'pending',
      },
      {
        label: 'Review Gate',
        detail: latestPrEvidence
          ? `PR #${latestPrEvidence.pr_number} · ${latestPrEvidence.approvals_count} approval(s)`
          : latestCommit
            ? `No PR evidence yet for ${latestCommit.short_sha}`
            : 'No commit available yet',
        status: pushStep?.status ?? 'pending',
      },
      {
        label: 'CI Gate',
        detail: latestPipelineRun
          ? `${latestPipelineRun.job_name || latestPipelineRun.pipeline_id} · ${latestPipelineRun.status}`
          : 'No Jenkins signal received yet',
        status: pipelineStep?.status ?? 'pending',
      },
      {
        label: 'Next Action',
        detail: currentFocus.nextAction,
        status: currentFocus.status === 'failed' ? 'failed' : currentFocus.status === 'warning' ? 'warning' : 'active',
      },
    ]
  }, [currentFocus, latestCommit, latestPipelineRun, latestPrEvidence, primaryTicket, primaryTicketId, steps])

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-surface-950">
      <div className="flex items-center justify-between border-b border-surface-800 bg-surface-900/60 px-3 py-1.5">
        <div className="flex items-center gap-2">
          <Activity size={12} className="text-surface-500" />
          <span className="text-[10px] font-medium uppercase tracking-wider text-surface-400">
            Pipeline Flow
          </span>
          {loading && <Loader2 size={11} className="animate-spin text-brand-400" />}
        </div>

        <div className="flex items-center gap-2 text-[8px] uppercase tracking-wider">
          <span
            className={`rounded border px-1.5 py-0.5 ${
              sseConnected
                ? 'border-success-500/30 bg-success-500/10 text-success-300'
                : 'border-surface-700 bg-surface-800 text-surface-500'
            }`}
          >
            {sseConnected ? 'Live' : 'Polling'}
          </span>
        </div>
      </div>

      {error && (
        <div className="flex items-center gap-2 border-b border-warning-500/20 bg-warning-500/10 px-3 py-1 text-[10px] text-warning-300">
          <AlertTriangle size={11} />
          <span className="truncate">{error}</span>
        </div>
      )}

      <div className="flex-1 min-h-0 overflow-hidden px-3 py-2">
        <div className="flex h-full min-h-0 flex-col gap-3">
          <div className="shrink-0 overflow-x-auto overflow-y-hidden">
            <div className="inline-flex min-w-full items-center">
              {steps.map((step, index) => {
                const style = STATUS_STYLE[step.status]
                const isActive = step.status === 'active'
                return (
                  <div key={step.key} className="flex items-center">
                    <div
                      className={`w-40 rounded-lg border px-2.5 py-2 ${style.card} ${
                        isActive ? 'animate-pulse' : ''
                      }`}
                    >
                      <div className="flex items-center gap-1.5">
                        <step.Icon size={12} className={style.icon} />
                        <span className="text-[9px] font-semibold uppercase tracking-wider">{step.label}</span>
                      </div>
                      <p className="mt-1.5 truncate font-mono text-[10px]">{step.detail}</p>
                    </div>

                    {index < steps.length - 1 && (
                      <div className="mx-2 flex items-center">
                        <div className={`h-px w-8 ${style.connector}`} />
                        <div className={`ml-1.5 h-1.5 w-1.5 rounded-full ${style.dot}`} />
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </div>

          <div className="grid flex-1 min-h-0 grid-cols-1 gap-3 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,0.9fr)_minmax(0,1fr)]">
            <div className="min-h-0 rounded-xl border border-white/6 bg-gradient-to-br from-white/[0.04] to-transparent p-3">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <currentFocus.Icon size={14} className={STATUS_STYLE[currentFocus.status].icon} />
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-surface-400">
                    Current Focus
                  </span>
                </div>
                <span className={`rounded border px-1.5 py-0.5 text-[8px] uppercase tracking-wider ${STATUS_STYLE[currentFocus.status].card}`}>
                  {currentFocus.status}
                </span>
              </div>
              <div className="mt-3">
                <p className="text-xs font-semibold text-surface-100">{currentFocus.label}</p>
                <p className="mt-1 text-[11px] text-surface-300">{currentFocus.detail}</p>
              </div>
              <div className="mt-4 rounded-lg border border-white/6 bg-surface-950/70 p-2.5">
                <p className="text-[9px] uppercase tracking-wider text-surface-500">Next action</p>
                <p className="mt-1 text-[11px] text-surface-200">{currentFocus.nextAction}</p>
              </div>
              <div className="mt-3 flex flex-wrap gap-1.5">
                {liveFeed.slice(0, 3).map((entry) => (
                  <span
                    key={entry.id}
                    className={`inline-flex items-center gap-1 rounded border px-2 py-0.5 text-[9px] ${STATUS_STYLE[entry.status].card}`}
                  >
                    <StatusGlyph status={entry.status} />
                    {entry.text}
                  </span>
                ))}
              </div>
            </div>

            <div className="min-h-0 rounded-xl border border-white/6 bg-white/[0.02] p-3">
              <div className="flex items-center gap-2">
                <Activity size={13} className="text-surface-400" />
                <span className="text-[10px] font-semibold uppercase tracking-wider text-surface-400">
                  Session Snapshot
                </span>
              </div>
              <div className="mt-3 grid grid-cols-2 gap-2">
                {snapshotMetrics.map((metric) => (
                  <div
                    key={metric.label}
                    className={`rounded-lg border p-2 ${STATUS_STYLE[metric.status].card}`}
                  >
                    <p className="text-[8px] uppercase tracking-wider opacity-70">{metric.label}</p>
                    <p className="mt-1 truncate font-mono text-[11px]">{metric.value}</p>
                  </div>
                ))}
              </div>
              <div className="mt-3 rounded-lg border border-white/6 bg-surface-950/70 p-2.5">
                <p className="text-[9px] uppercase tracking-wider text-surface-500">Latest commit summary</p>
                <p className="mt-1 truncate text-[11px] text-surface-200">
                  {latestCommit ? latestCommit.summary || '(no message)' : 'No local commit in context'}
                </p>
              </div>
            </div>

            <div className="min-h-0 rounded-xl border border-white/6 bg-white/[0.02] p-3">
              <div className="flex items-center gap-2">
                <ShieldCheck size={13} className="text-surface-400" />
                <span className="text-[10px] font-semibold uppercase tracking-wider text-surface-400">
                  Gates / Blockers
                </span>
              </div>
              <div className="mt-3 space-y-2">
                {gateItems.map((item) => (
                  <div
                    key={item.label}
                    className={`rounded-lg border px-2.5 py-2 ${STATUS_STYLE[item.status].card}`}
                  >
                    <div className="flex items-center gap-1.5">
                      <StatusGlyph status={item.status} />
                      <span className="text-[9px] font-semibold uppercase tracking-wider">{item.label}</span>
                    </div>
                    <p className="mt-1 text-[11px] text-surface-200">{item.detail}</p>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="border-t border-surface-800 bg-surface-900/50 px-3 py-1.5">
        {liveFeed.length === 0 ? (
          <div className="text-[9px] uppercase tracking-wider text-surface-600">
            Waiting for local activity...
          </div>
        ) : (
          <div className="flex items-center gap-2 overflow-x-auto whitespace-nowrap">
            {liveFeed.map((entry) => {
              const style = STATUS_STYLE[entry.status]
              return (
                <span
                  key={entry.id}
                  className={`inline-flex items-center gap-1 rounded border px-2 py-0.5 text-[9px] ${style.card}`}
                >
                  {entry.status === 'success' ? (
                    <CheckCircle2 size={9} />
                  ) : entry.status === 'active' ? (
                    <Loader2 size={9} className="animate-spin" />
                  ) : (
                    <CircleDashed size={9} />
                  )}
                  {entry.text}
                </span>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
