import { render } from '@testing-library/react'
import { Spinner } from '@/components/shared/Spinner'

describe('Spinner', () => {
  it('renders an animated div', () => {
    const { container } = render(<Spinner />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('animate-spin')
  })

  it('defaults to medium size', () => {
    const { container } = render(<Spinner />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('w-6')
    expect(el.className).toContain('h-6')
  })

  it('applies small size', () => {
    const { container } = render(<Spinner size="sm" />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('w-4')
    expect(el.className).toContain('h-4')
  })

  it('applies large size', () => {
    const { container } = render(<Spinner size="lg" />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('w-8')
    expect(el.className).toContain('h-8')
  })

  it('merges custom className', () => {
    const { container } = render(<Spinner className="extra" />)
    const el = container.firstChild as HTMLElement
    expect(el.className).toContain('extra')
  })

  it('has transparent top border for animation effect', () => {
    const { container } = render(<Spinner />)
    const el = container.firstChild as HTMLElement
    expect(el.style.borderTopColor).toBe('transparent')
  })
})
