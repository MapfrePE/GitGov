import { useEffect, useState } from 'react'
import { Download, RefreshCw, FileJson } from 'lucide-react'
import { useControlPlaneStore, type ExportLogEntry } from '@/store/useControlPlaneStore'
import { Button } from '@/components/shared/Button'

function formatTimestamp(ms: number | null | undefined): string {
  if (!ms) return '—'
  return new Date(ms).toLocaleDateString('es', {
    year: 'numeric', month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit',
  })
}

function fromDateInputValue(s: string): number | undefined {
  if (!s) return undefined
  return new Date(s).getTime()
}

export function ExportPanel() {
  const { exportLogs, exportAuditData, loadExportLogs } = useControlPlaneStore()
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [isExporting, setIsExporting] = useState(false)

  useEffect(() => {
    void loadExportLogs()
  }, [loadExportLogs])

  const handleExport = async () => {
    setIsExporting(true)
    try {
      const result = await exportAuditData({
        exportType: 'events',
        startDate: fromDateInputValue(startDate),
        endDate: fromDateInputValue(endDate),
      })

      if (!result?.data) {
        return
      }

      const content = JSON.stringify(result.data, null, 2)
      const blob = new Blob([content], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `gitgov-export-${new Date().toISOString().split('T')[0]}.json`
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
    } catch {
      // Error displayed via store
    } finally {
      setIsExporting(false)
    }
  }

  return (
    <div className="glass-panel p-5">
      <div className="flex items-center gap-2 mb-4">
        <FileJson size={14} className="text-surface-400" />
        <span className="card-header">Exportar Historial de Auditoría</span>
      </div>

      <div className="flex flex-wrap gap-3 items-end mb-4">
        <div className="flex flex-col gap-1">
          <label htmlFor="export-start-date" className="text-[10px] text-surface-500">Desde</label>
          <input
            id="export-start-date"
            type="date"
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className="bg-surface-800 border border-surface-600 rounded px-2 py-1 text-xs text-surface-200 focus:outline-none focus:border-surface-400"
          />
        </div>
        <div className="flex flex-col gap-1">
          <label htmlFor="export-end-date" className="text-[10px] text-surface-500">Hasta</label>
          <input
            id="export-end-date"
            type="date"
            value={endDate}
            onChange={(e) => setEndDate(e.target.value)}
            className="bg-surface-800 border border-surface-600 rounded px-2 py-1 text-xs text-surface-200 focus:outline-none focus:border-surface-400"
          />
        </div>
        <Button
          onClick={() => void handleExport()}
          loading={isExporting}
          className="flex items-center gap-1.5"
        >
          <Download size={12} />
          Exportar JSON
        </Button>
      </div>

      {/* Export history */}
      <div className="border-t border-surface-700 pt-3">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] text-surface-500 font-medium uppercase tracking-wide">
            Historial de exports
          </span>
          <button
            onClick={() => void loadExportLogs()}
            className="p-0.5 text-surface-600 hover:text-surface-300 transition-colors"
            title="Refrescar"
          >
            <RefreshCw size={11} />
          </button>
        </div>

        {exportLogs.length === 0 ? (
          <p className="text-xs text-surface-600 py-3 text-center">Sin exports anteriores</p>
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-surface-800">
                <th className="text-left pb-1.5 text-surface-500 font-medium">Exportado por</th>
                <th className="text-left pb-1.5 text-surface-500 font-medium">Registros</th>
                <th className="text-left pb-1.5 text-surface-500 font-medium">Desde</th>
                <th className="text-left pb-1.5 text-surface-500 font-medium">Hasta</th>
                <th className="text-left pb-1.5 text-surface-500 font-medium">Fecha</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-surface-800">
              {exportLogs.map((log: ExportLogEntry) => (
                <tr key={log.id} className="hover:bg-surface-800/20">
                  <td className="py-1.5 pr-2 font-mono text-surface-300">{log.exported_by}</td>
                  <td className="py-1.5 pr-2 text-surface-300">{log.record_count.toLocaleString()}</td>
                  <td className="py-1.5 pr-2 text-surface-400">
                    {formatTimestamp(log.date_range_start)}
                  </td>
                  <td className="py-1.5 pr-2 text-surface-400">
                    {formatTimestamp(log.date_range_end)}
                  </td>
                  <td className="py-1.5 text-surface-400">{formatTimestamp(log.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
