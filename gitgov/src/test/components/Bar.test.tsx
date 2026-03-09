import { render } from '@testing-library/react'
import { Bar } from '@/components/control_plane/Bar'

describe('Bar', () => {
  it('renders a bar with default brand color', () => {
    const { container } = render(<Bar value={50} />)
    const inner = container.querySelector('[style]') as HTMLElement
    expect(inner).toBeInTheDocument()
    expect(inner.style.width).toBe('50%')
    expect(inner.className).toContain('bg-brand-500')
  })

  it('renders with success color', () => {
    const { container } = render(<Bar value={75} color="success" />)
    const inner = container.querySelector('[style]') as HTMLElement
    expect(inner.className).toContain('bg-success-500')
  })

  it('clamps value at 100%', () => {
    const { container } = render(<Bar value={150} />)
    const inner = container.querySelector('[style]') as HTMLElement
    expect(inner.style.width).toBe('100%')
  })

  it('renders 0% bar', () => {
    const { container } = render(<Bar value={0} />)
    const inner = container.querySelector('[style]') as HTMLElement
    expect(inner.style.width).toBe('0%')
  })
})
