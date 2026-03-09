import { render, screen, fireEvent } from '@testing-library/react'
import { Button } from '@/components/shared/Button'

describe('Button', () => {
  it('renders children text', () => {
    render(<Button>Click me</Button>)
    expect(screen.getByRole('button', { name: 'Click me' })).toBeInTheDocument()
  })

  it('defaults to type="button"', () => {
    render(<Button>OK</Button>)
    expect(screen.getByRole('button')).toHaveAttribute('type', 'button')
  })

  it('respects explicit type="submit"', () => {
    render(<Button type="submit">Send</Button>)
    expect(screen.getByRole('button')).toHaveAttribute('type', 'submit')
  })

  it('is disabled when disabled prop is true', () => {
    render(<Button disabled>Nope</Button>)
    expect(screen.getByRole('button')).toBeDisabled()
  })

  it('is disabled when loading is true', () => {
    render(<Button loading>Wait</Button>)
    expect(screen.getByRole('button')).toBeDisabled()
  })

  it('shows spinner when loading', () => {
    const { container } = render(<Button loading>Loading</Button>)
    // Spinner renders an animated div
    expect(container.querySelector('.animate-spin')).toBeInTheDocument()
  })

  it('does not show spinner when not loading', () => {
    const { container } = render(<Button>Normal</Button>)
    expect(container.querySelector('.animate-spin')).not.toBeInTheDocument()
  })

  it('fires onClick handler', () => {
    const onClick = vi.fn()
    render(<Button onClick={onClick}>Go</Button>)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('does not fire onClick when disabled', () => {
    const onClick = vi.fn()
    render(<Button disabled onClick={onClick}>No</Button>)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).not.toHaveBeenCalled()
  })

  it('applies variant classes', () => {
    const { rerender } = render(<Button variant="primary">P</Button>)
    expect(screen.getByRole('button').className).toContain('bg-brand-600')

    rerender(<Button variant="danger">D</Button>)
    expect(screen.getByRole('button').className).toContain('bg-danger-600')

    rerender(<Button variant="ghost">G</Button>)
    expect(screen.getByRole('button').className).toContain('hover:bg-surface-700')
  })

  it('applies size classes', () => {
    const { rerender } = render(<Button size="sm">S</Button>)
    expect(screen.getByRole('button').className).toContain('px-3')

    rerender(<Button size="lg">L</Button>)
    expect(screen.getByRole('button').className).toContain('px-6')
  })

  it('merges custom className', () => {
    render(<Button className="my-custom">C</Button>)
    expect(screen.getByRole('button').className).toContain('my-custom')
  })
})
