export function Bar({ value, color = 'brand' }: { value: number; color?: 'brand' | 'success' }) {
  const bg = color === 'success' ? 'bg-success-500/70' : 'bg-brand-500/70'
  return (
    <div className="h-1 bg-white/4 rounded-full overflow-hidden">
      <div className={`h-full ${bg} rounded-full transition-all duration-700`} style={{ width: `${Math.min(100, value)}%` }} />
    </div>
  )
}
