import { render, screen } from '@testing-library/react'
import { Badge } from '@/components/shared/Badge'

describe('Badge', () => {
  it('renders children text', () => {
    render(<Badge>Active</Badge>)
    expect(screen.getByText('Active')).toBeInTheDocument()
  })

  it('defaults to neutral variant', () => {
    render(<Badge>Default</Badge>)
    expect(screen.getByText('Default').className).toContain('bg-surface-600')
  })

  it('applies success variant classes', () => {
    render(<Badge variant="success">OK</Badge>)
    expect(screen.getByText('OK').className).toContain('text-success-400')
  })

  it('applies warning variant classes', () => {
    render(<Badge variant="warning">Warn</Badge>)
    expect(screen.getByText('Warn').className).toContain('text-warning-400')
  })

  it('applies danger variant classes', () => {
    render(<Badge variant="danger">Error</Badge>)
    expect(screen.getByText('Error').className).toContain('text-danger-400')
  })

  it('applies info variant classes', () => {
    render(<Badge variant="info">Info</Badge>)
    expect(screen.getByText('Info').className).toContain('text-brand-400')
  })

  it('renders as a span element', () => {
    render(<Badge>Tag</Badge>)
    expect(screen.getByText('Tag').tagName).toBe('SPAN')
  })

  it('merges custom className', () => {
    render(<Badge className="extra-class">Custom</Badge>)
    expect(screen.getByText('Custom').className).toContain('extra-class')
  })
})
