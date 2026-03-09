import { render, screen } from '@testing-library/react'
import { Skeleton, SkeletonFileRow, SkeletonLogRow, SkeletonDashboard } from '@/components/shared/Skeleton'

describe('Skeleton', () => {
  it('renders a pulsing div', () => {
    const { container } = render(<Skeleton />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('animate-pulse')
  })

  it('merges custom className', () => {
    const { container } = render(<Skeleton className="h-4 w-32" />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('h-4')
    expect(el.className).toContain('w-32')
  })
})

describe('SkeletonFileRow', () => {
  it('renders skeleton elements', () => {
    const { container } = render(<SkeletonFileRow />)
    const pulses = container.querySelectorAll('.animate-pulse')
    expect(pulses.length).toBeGreaterThanOrEqual(3)
  })
})

describe('SkeletonLogRow', () => {
  it('renders skeleton elements', () => {
    const { container } = render(<SkeletonLogRow />)
    const pulses = container.querySelectorAll('.animate-pulse')
    expect(pulses.length).toBeGreaterThanOrEqual(3)
  })
})

describe('SkeletonDashboard', () => {
  it('renders 4 stat cards', () => {
    const { container } = render(<SkeletonDashboard />)
    // 4 stat cards + 5 log rows, each with multiple skeletons
    const pulses = container.querySelectorAll('.animate-pulse')
    expect(pulses.length).toBeGreaterThanOrEqual(8)
  })
})
