import { create } from 'zustand'
import { X, CheckCircle, AlertCircle, AlertTriangle, Info } from 'lucide-react'
import clsx from 'clsx'
import { useEffect } from 'react'

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

const colorMap = {
  success: 'bg-success-600',
  error: 'bg-danger-600',
  warning: 'bg-warning-600',
  info: 'bg-brand-600',
}

function ToastItem({ toast: t }: { toast: Toast }) {
  const { removeToast } = useToastStore()
  const Icon = iconMap[t.type]

  useEffect(() => {
    if (t.type !== 'error') {
      const timer = setTimeout(() => removeToast(t.id), 5000)
      return () => clearTimeout(timer)
    }
  }, [t.id, t.type, removeToast])

  return (
    <div
      className={clsx(
        'flex items-center gap-3 px-4 py-3 rounded-lg text-white shadow-lg',
        colorMap[t.type]
      )}
    >
      <Icon size={20} />
      <span className="flex-1">{t.message}</span>
      <button onClick={() => removeToast(t.id)} className="hover:opacity-70">
        <X size={16} />
      </button>
    </div>
  )
}

export function ToastContainer() {
  const { toasts } = useToastStore()

  return (
    <div className="fixed bottom-4 right-4 flex flex-col gap-2 z-50">
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} />
      ))}
    </div>
  )
}
