import { useState, memo } from 'react'
import { format, formatDistanceToNow } from 'date-fns'
import { es } from 'date-fns/locale'
import type { AuditLogEntry } from '@/lib/types'
import { Badge } from '@/components/shared/Badge'
import { ChevronDown, ChevronUp, FileText, GitCommit } from 'lucide-react'

interface AuditLogRowProps {
  log: AuditLogEntry
}

export const AuditLogRow = memo(function AuditLogRow({ log }: AuditLogRowProps) {
  const [expanded, setExpanded] = useState(false)

  const timestamp = new Date(log.timestamp)
  const isRecent = Date.now() - log.timestamp < 24 * 60 * 60 * 1000
  const formattedDate = isRecent
    ? formatDistanceToNow(timestamp, { addSuffix: true, locale: es })
    : format(timestamp, 'dd/MM/yyyy HH:mm')

  const statusVariant = {
    Success: 'success' as const,
    Blocked: 'danger' as const,
    Failed: 'warning' as const,
  }[log.status]

  const actionVariant = {
    Push: 'neutral' as const,
    BranchCreate: 'neutral' as const,
    StageFile: 'neutral' as const,
    Commit: 'neutral' as const,
    BlockedPush: 'danger' as const,
    BlockedBranch: 'danger' as const,
  }[log.action]

  return (
    <>
      <tr
        className="border-b border-surface-700 hover:bg-surface-800/50 cursor-pointer"
        onClick={() => setExpanded(!expanded)}
      >
        <td className="px-4 py-3 text-sm text-surface-300">{formattedDate}</td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            {log.developer_name && (
              <span className="text-sm text-white">{log.developer_name}</span>
            )}
            <span className="text-xs text-surface-500">@{log.developer_login}</span>
          </div>
        </td>
        <td className="px-4 py-3">
          <Badge variant={actionVariant}>{log.action}</Badge>
        </td>
        <td className="px-4 py-3 text-sm text-white font-mono">{log.branch}</td>
        <td className="px-4 py-3 text-sm text-surface-400">
          {log.files.length > 0 ? (
            <span className="flex items-center gap-1">
              <FileText size={12} />
              {log.files.length} archivo{log.files.length !== 1 ? 's' : ''}
            </span>
          ) : (
            '-'
          )}
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            <Badge variant={statusVariant}>{log.status}</Badge>
            {log.reason && (
              <span className="text-surface-400">
                {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
              </span>
            )}
          </div>
        </td>
      </tr>
      {expanded && log.reason && (
        <tr className="bg-surface-800/30">
          <td colSpan={6} className="px-4 py-3">
            <div className="text-sm">
              <p className="text-surface-400 mb-2">Razón del bloqueo/fallo:</p>
              <p className="text-danger-400 bg-danger-500/10 p-2 rounded">{log.reason}</p>
            </div>
            {log.files.length > 0 && (
              <div className="mt-2">
                <p className="text-surface-400 text-xs mb-1">Archivos involucrados:</p>
                <ul className="text-xs text-surface-500 font-mono">
                  {log.files.map((f, i) => (
                    <li key={i}>{f}</li>
                  ))}
                </ul>
              </div>
            )}
            {log.commit_hash && (
              <div className="mt-2 flex items-center gap-1 text-xs text-surface-500">
                <GitCommit size={12} />
                <span className="font-mono">{log.commit_hash}</span>
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  )
})
