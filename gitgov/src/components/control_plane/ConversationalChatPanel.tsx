import { useEffect, useRef, useState } from 'react'
import { useControlPlaneStore, type ChatAskResponse, type ChatMessage } from '@/store/useControlPlaneStore'
import { formatTimeOnly } from '@/lib/timezone'

// ── Suggestion chips ─────────────────────────────────────────────────────────

const SUGGESTIONS = [
  '¿Quién hizo push a main esta semana sin ticket de Jira?',
  '¿Cuántos pushes bloqueados tuvo el equipo este mes?',
  'Muéstrame todos los commits de dev1 entre 2026-01-01 y 2026-03-01',
]

// ── Status badge ──────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: ChatAskResponse['status'] }) {
  const map = {
    ok: { label: 'OK', color: 'text-emerald-400 border-emerald-400/40 bg-emerald-400/8' },
    insufficient_data: { label: 'DATOS INSUFICIENTES', color: 'text-amber-400 border-amber-400/40 bg-amber-400/8' },
    feature_not_available: { label: 'CAPACIDAD FALTANTE', color: 'text-sky-400 border-sky-400/40 bg-sky-400/8' },
    error: { label: 'ERROR', color: 'text-rose-400 border-rose-400/40 bg-rose-400/8' },
  } as const

  const { label, color } = map[status] ?? map.error
  return (
    <span
      className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[9px] font-mono tracking-widest uppercase ${color}`}
    >
      {label}
    </span>
  )
}

// ── Data refs pill ────────────────────────────────────────────────────────────

function DataRefs({ refs }: { refs: string[] }) {
  if (!refs.length) return null
  return (
    <div className="flex flex-wrap gap-1 mt-2">
      {refs.map((r) => (
        <span key={r} className="text-[9px] font-mono text-surface-500 bg-white/4 border border-white/6 rounded px-1.5 py-0.5">
          {r}
        </span>
      ))}
    </div>
  )
}

// ── User message bubble ───────────────────────────────────────────────────────

function UserBubble({ msg, displayTimezone }: { msg: ChatMessage; displayTimezone: string }) {
  return (
    <div className="flex items-start gap-2 justify-end">
      <div className="max-w-[82%]">
        <div className="bg-brand-500/15 border border-brand-500/25 rounded-lg rounded-tr-sm px-3 py-2">
          <p className="text-[11px] font-mono text-brand-200 leading-relaxed">{msg.content}</p>
        </div>
        <p className="text-[9px] text-surface-600 font-mono mt-0.5 text-right">
          {formatTimeOnly(msg.timestamp, displayTimezone)}
        </p>
      </div>
      <div className="w-5 h-5 rounded-sm bg-brand-500/25 border border-brand-500/30 flex items-center justify-center shrink-0 mt-0.5">
        <span className="text-[8px] font-mono text-brand-400">A</span>
      </div>
    </div>
  )
}

// ── Assistant message bubble ─────────────────────────────────────────────────

function AssistantBubble({ msg, onReport, displayTimezone }: { msg: ChatMessage; onReport: (msg: ChatMessage) => void; displayTimezone: string }) {
  const r = msg.response
  const [reported, setReported] = useState(false)

  const handleReport = async () => {
    setReported(true)
    onReport(msg)
  }

  return (
    <div className="flex items-start gap-2">
      <div className="w-5 h-5 rounded-sm bg-surface-700 border border-white/10 flex items-center justify-center shrink-0 mt-0.5">
        <span className="text-[8px] font-mono text-surface-400">G</span>
      </div>
      <div className="max-w-[88%] flex-1">
        <div className="bg-surface-800/80 border border-white/8 rounded-lg rounded-tl-sm px-3 py-2.5">
          {r && (
            <div className="flex items-center gap-2 mb-2">
              <StatusBadge status={r.status} />
              {r.status === 'ok' && (
                <span className="text-[9px] font-mono text-surface-600">
                  {r.data_refs.join(' · ')}
                </span>
              )}
            </div>
          )}
          <p className="text-[11px] text-surface-200 leading-relaxed whitespace-pre-wrap font-mono">
            {msg.content}
          </p>
          {r?.status === 'ok' && <DataRefs refs={r.data_refs} />}
          {r?.status === 'feature_not_available' && r.can_report_feature && (
            <div className="mt-3 pt-2 border-t border-white/6">
              {r.missing_capability && (
                <p className="text-[9px] font-mono text-surface-500 mb-2">
                  capacidad: <span className="text-sky-400/80">{r.missing_capability}</span>
                </p>
              )}
              {!reported ? (
                <button
                  type="button"
                  onClick={handleReport}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded border border-sky-500/40 bg-sky-500/10 text-sky-400 text-[10px] font-mono tracking-wide hover:bg-sky-500/20 hover:border-sky-500/60 transition-colors"
                >
                  <span className="text-[8px]">▶</span>
                  Reportar esta necesidad
                </button>
              ) : (
                <span className="text-[10px] font-mono text-emerald-400">✓ Solicitud enviada</span>
              )}
            </div>
          )}
        </div>
        <p className="text-[9px] text-surface-600 font-mono mt-0.5">
          {formatTimeOnly(msg.timestamp, displayTimezone)}
        </p>
      </div>
    </div>
  )
}

// ── Typing indicator ──────────────────────────────────────────────────────────

function TypingIndicator() {
  return (
    <div className="flex items-start gap-2">
      <div className="w-5 h-5 rounded-sm bg-surface-700 border border-white/10 flex items-center justify-center shrink-0">
        <span className="text-[8px] font-mono text-surface-400">G</span>
      </div>
      <div className="bg-surface-800/80 border border-white/8 rounded-lg rounded-tl-sm px-3 py-2.5">
        <div className="flex items-center gap-1">
          {[0, 1, 2].map((i) => (
            <span
              key={i}
              className="w-1 h-1 rounded-full bg-surface-500 animate-bounce"
              style={{ animationDelay: `${i * 150}ms` }}
            />
          ))}
        </div>
      </div>
    </div>
  )
}

// ── Suggestion chip ───────────────────────────────────────────────────────────

function SuggestionChip({ text, onClick }: { text: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="text-left text-[10px] font-mono text-surface-400 bg-white/3 hover:bg-white/6 border border-white/8 hover:border-white/15 rounded px-2.5 py-1.5 transition-colors leading-snug"
    >
      {text}
    </button>
  )
}

// ── Main component ────────────────────────────────────────────────────────────

export function ConversationalChatPanel() {
  const chatSessions = useControlPlaneStore((s) => s.chatSessions)
  const activeChatSessionId = useControlPlaneStore((s) => s.activeChatSessionId)
  const chatMessages = useControlPlaneStore((s) => s.chatMessages)
  const isChatLoading = useControlPlaneStore((s) => s.isChatLoading)
  const chatAsk = useControlPlaneStore((s) => s.chatAsk)
  const reportFeature = useControlPlaneStore((s) => s.reportFeature)
  const clearChatMessages = useControlPlaneStore((s) => s.clearChatMessages)
  const createChatSession = useControlPlaneStore((s) => s.createChatSession)
  const setActiveChatSession = useControlPlaneStore((s) => s.setActiveChatSession)
  const closeChatSession = useControlPlaneStore((s) => s.closeChatSession)
  const isConnected = useControlPlaneStore((s) => s.isConnected)
  const userRole = useControlPlaneStore((s) => s.userRole)
  const selectedOrgName = useControlPlaneStore((s) => s.selectedOrgName)
  const displayTimezone = useControlPlaneStore((s) => s.displayTimezone)

  const [input, setInput] = useState('')
  const bottomRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const isAdmin = userRole === 'Admin'

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'auto' })
  }, [chatMessages, isChatLoading])

  const handleSubmit = async () => {
    const q = input.trim()
    if (!q || isChatLoading) return
    setInput('')
    await chatAsk(q, selectedOrgName)
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      void handleSubmit()
    }
  }

  const handleReport = async (msg: ChatMessage) => {
    if (!msg.response) return
    await reportFeature(
      chatMessages.find((m) => m.role === 'user' && m.timestamp < msg.timestamp)?.content ?? '',
      msg.response.missing_capability ?? undefined,
    )
  }

  if (!isConnected) return null
  if (!isAdmin) return null

  const isEmpty = chatMessages.length === 0

  return (
    <div className="border border-white/8 rounded-xl overflow-hidden bg-surface-900/60 backdrop-blur-sm">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-white/6 bg-surface-800/50">
        <div className="flex items-center gap-2.5">
          <div className="flex gap-1">
            <span className="w-2 h-2 rounded-full bg-rose-500/70" />
            <span className="w-2 h-2 rounded-full bg-amber-500/70" />
            <span className="w-2 h-2 rounded-full bg-emerald-500/70" />
          </div>
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] font-mono text-surface-400 tracking-wide">gitgov</span>
            <span className="text-[11px] font-mono text-surface-600">/</span>
            <span className="text-[11px] font-mono text-surface-300 tracking-wide">governance-query</span>
          </div>
        </div>
        {!isEmpty && (
          <button
            type="button"
            onClick={clearChatMessages}
            className="text-[9px] font-mono text-surface-600 hover:text-surface-400 transition-colors tracking-wide"
          >
            clear
          </button>
        )}
      </div>

      {/* Sessions tabs */}
      <div className="px-3 py-2 border-b border-white/6 bg-surface-900/40">
        <div className="flex items-center gap-1.5 overflow-x-auto">
          {chatSessions.map((session, idx) => {
            const isActive = session.id === activeChatSessionId
            return (
              <div
                key={session.id}
                className={`group flex items-center gap-1 rounded border px-2 py-1 shrink-0 ${
                  isActive
                    ? 'border-brand-500/40 bg-brand-500/10'
                    : 'border-white/10 bg-white/3'
                }`}
              >
                <button
                  type="button"
                  onClick={() => setActiveChatSession(session.id)}
                  disabled={isChatLoading}
                  className={`max-w-[180px] truncate text-[10px] font-mono transition-colors ${
                    isActive ? 'text-brand-300' : 'text-surface-400 hover:text-surface-200'
                  } disabled:opacity-50`}
                  title={session.title || `Chat ${idx + 1}`}
                >
                  {session.title || `Chat ${idx + 1}`}
                </button>
                <button
                  type="button"
                  onClick={() => closeChatSession(session.id)}
                  disabled={isChatLoading}
                  className="text-[10px] text-surface-600 hover:text-rose-300 transition-colors disabled:opacity-40"
                  title="Cerrar conversación"
                  aria-label={`Cerrar conversación ${idx + 1}`}
                >
                  x
                </button>
              </div>
            )
          })}
          <button
            type="button"
            onClick={createChatSession}
            disabled={isChatLoading}
            className="shrink-0 rounded border border-white/12 bg-white/4 px-2 py-1 text-[11px] font-mono text-surface-400 hover:text-surface-200 hover:border-white/20 transition-colors disabled:opacity-50"
            title="Nueva conversación"
            aria-label="Nueva conversación"
          >
            +
          </button>
        </div>
      </div>

      {/* Body */}
      <div className="min-h-[220px] max-h-[520px] overflow-y-auto p-4 space-y-4 scroll-smooth">
        {isEmpty ? (
          <div className="flex flex-col gap-3 py-2">
            <div className="flex items-center gap-2">
              <span className="text-[10px] font-mono text-brand-500">›</span>
              <p className="text-[11px] font-mono text-surface-400">
                Haz una pregunta sobre gobernanza Git…
              </p>
            </div>
            <div className="flex flex-col gap-1.5 pl-4">
              {SUGGESTIONS.map((s) => (
                <SuggestionChip key={s} text={s} onClick={() => setInput(s)} />
              ))}
            </div>
            <p className="text-[9px] font-mono text-surface-600 pl-4 mt-1">
              Shift+Enter para salto de línea · Enter para enviar
            </p>
          </div>
        ) : (
          chatMessages.map((msg) =>
            msg.role === 'user' ? (
              <UserBubble key={msg.id} msg={msg} displayTimezone={displayTimezone} />
            ) : (
              <AssistantBubble key={msg.id} msg={msg} onReport={handleReport} displayTimezone={displayTimezone} />
            ),
          )
        )}
        {isChatLoading && <TypingIndicator />}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="border-t border-white/6 bg-surface-800/40 p-3">
        <div className="flex items-end gap-2">
          <span className="text-[11px] font-mono text-brand-500 pb-2 select-none">›_</span>
          <textarea
            ref={textareaRef}
            rows={1}
            value={input}
            onChange={(e) => {
              setInput(e.target.value)
              // Auto-grow: max 4 rows
              e.target.style.height = 'auto'
              e.target.style.height = `${Math.min(e.target.scrollHeight, 88)}px`
            }}
            onKeyDown={handleKeyDown}
            placeholder="Escribe tu pregunta de gobernanza…"
            disabled={isChatLoading}
            className="flex-1 resize-none bg-transparent text-[11px] font-mono text-surface-200 placeholder:text-surface-600 outline-none leading-relaxed disabled:opacity-50"
            style={{ minHeight: '24px', maxHeight: '88px' }}
          />
          <button
            type="button"
            onClick={() => void handleSubmit()}
            disabled={!input.trim() || isChatLoading}
            className="shrink-0 w-7 h-7 rounded border border-brand-500/40 bg-brand-500/10 text-brand-400 flex items-center justify-center hover:bg-brand-500/20 hover:border-brand-500/60 disabled:opacity-30 disabled:cursor-not-allowed transition-colors mb-0.5"
            aria-label="Enviar"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M1 9L9 1M9 1H3M9 1V7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </button>
        </div>
        {!isEmpty && (
          <div className="flex flex-wrap gap-1 mt-2 pl-5">
            {SUGGESTIONS.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setInput(s)}
                className="text-[9px] font-mono text-surface-600 hover:text-surface-400 bg-white/2 hover:bg-white/5 border border-white/5 rounded px-1.5 py-0.5 transition-colors truncate max-w-[200px]"
              >
                {s.slice(0, 40)}{s.length > 40 ? '…' : ''}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
