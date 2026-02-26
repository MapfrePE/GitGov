import clsx from 'clsx'

interface SkeletonProps {
  className?: string
}

export function Skeleton({ className }: SkeletonProps) {
  return (
    <div
      className={clsx(
        'animate-pulse rounded bg-surface-700/40',
        className
      )}
    />
  )
}

export function SkeletonFileRow() {
  return (
    <div className="flex items-center gap-2.5 px-3 py-2.5">
      <Skeleton className="w-3.5 h-3.5 rounded" />
      <Skeleton className="w-4 h-4 rounded" />
      <div className="flex-1 space-y-1">
        <Skeleton className="h-3 w-3/4" />
      </div>
    </div>
  )
}

export function SkeletonLogRow() {
  return (
    <div className="flex items-center gap-3 px-4 py-3">
      <Skeleton className="w-5 h-5 rounded-full" />
      <div className="flex-1 space-y-1.5">
        <Skeleton className="h-3 w-1/3" />
        <Skeleton className="h-2.5 w-2/3" />
      </div>
      <Skeleton className="h-3 w-16" />
    </div>
  )
}

export function SkeletonDashboard() {
  return (
    <div className="p-6 space-y-6">
      <div className="flex gap-4">
        {[1, 2, 3, 4].map((i) => (
          <div key={i} className="flex-1 p-4 rounded-lg bg-surface-800/50 space-y-3">
            <Skeleton className="h-3 w-20" />
            <Skeleton className="h-8 w-16" />
          </div>
        ))}
      </div>
      <div className="space-y-2">
        {[1, 2, 3, 4, 5].map((i) => (
          <SkeletonLogRow key={i} />
        ))}
      </div>
    </div>
  )
}
