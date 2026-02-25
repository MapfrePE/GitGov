import { forwardRef } from 'react'
import type { HTMLAttributes } from 'react'
import clsx from 'clsx'

type BadgeVariant = 'success' | 'warning' | 'danger' | 'neutral' | 'info'

interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant
}

const variantClasses: Record<BadgeVariant, string> = {
  success: 'bg-success-500/15 text-success-400 ring-1 ring-success-500/20',
  warning: 'bg-warning-500/15 text-warning-400 ring-1 ring-warning-500/20',
  danger: 'bg-danger-500/15 text-danger-400 ring-1 ring-danger-500/20',
  neutral: 'bg-surface-600/40 text-surface-300 ring-1 ring-surface-500/20',
  info: 'bg-brand-500/15 text-brand-400 ring-1 ring-brand-500/20',
}

export const Badge = forwardRef<HTMLSpanElement, BadgeProps>(
  ({ variant = 'neutral', className, children, ...props }, ref) => {
    return (
      <span
        ref={ref}
        className={clsx(
          'inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium',
          variantClasses[variant],
          className
        )}
        {...props}
      >
        {children}
      </span>
    )
  }
)

Badge.displayName = 'Badge'
