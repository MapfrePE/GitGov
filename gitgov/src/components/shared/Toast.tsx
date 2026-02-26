/* eslint-disable react-refresh/only-export-components */
import { create } from 'zustand'
import { X, CheckCircle, AlertCircle, AlertTriangle, Info } from 'lucide-react'
import clsx from 'clsx'
import { useEffect, useState } from 'react'

type ToastType = 'success' | 'error' | 'warning' | 'info'

interface Toast {
  id: string
  type: ToastType
  message: string
}

interface ToastStore {
  toasts: Toast[]
  addToast: (type: ToastType, message: string) => void
  removeToast: (id: string) => void
}

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  addToast: (type, message) => {
    const id = Math.random().toString(36).substring(7)
    set((state) => ({ toasts: [...state.toasts, { id, type, message }] }))
  },
  removeToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }))
  },
}))

export function toast(type: ToastType, message: string) {
  useToastStore.getState().addToast(type, message)
}

const iconMap = {
  success: CheckCircle,
  error: AlertCircle,
  warning: AlertTriangle,
  info: Info,
}

const styleMap = {
  success: 'border-success-500/30 bg-surface-800/95 text-success-400',
  error: 'border-danger-500/30 bg-surface-800/95 text-danger-400',
  warning: 'border-warning-500/30 bg-surface-800/95 text-warning-400',
  info: 'border-brand-500/30 bg-surface-800/95 text-brand-400',
}

function ToastItem({ toast: t }: { toast: Toast }) {
  const { removeToast } = useToastStore()
  const Icon = iconMap[t.type]
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    requestAnimationFrame(() => setVisible(true))
  }, [])

  useEffect(() => {
    if (t.type !== 'error') {
      const timer = setTimeout(() => removeToast(t.id), 4000)
      return () => clearTimeout(timer)
    }
  }, [t.id, t.type, removeToast])

  return (
    <div
      className={clsx(
        'flex items-center gap-2.5 px-3.5 py-2.5 rounded-lg border shadow-lg backdrop-blur-sm transition-all duration-300 min-w-65 max-w-95',
        styleMap[t.type],
        visible ? 'translate-x-0 opacity-100' : 'translate-x-4 opacity-0'
      )}
    >
      <Icon size={16} className="shrink-0" />
      <span className="flex-1 text-xs text-surface-200 leading-snug">{t.message}</span>
      <button onClick={() => removeToast(t.id)} className="shrink-0 text-surface-500 hover:text-surface-300 transition-colors">
        <X size={14} />
      </button>
    </div>
  )
}

export function ToastContainer() {
  const { toasts } = useToastStore()

  return (
    <div className="fixed top-16 right-5 flex flex-col gap-2 z-50 pointer-events-none">
      {toasts.map((t) => (
        <div key={t.id} className="pointer-events-auto">
          <ToastItem toast={t} />
        </div>
      ))}
    </div>
  )
}
