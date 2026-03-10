import { render, screen, fireEvent } from '@testing-library/react'
import type { ReactElement } from 'react'
import { ErrorBoundary } from '@/components/shared/ErrorBoundary'

function ThrowingComponent({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) throw new Error('Test explosion')
  return <div>All good</div>
}

describe('ErrorBoundary', () => {
  // Suppress console.error from React's error boundary logging
  const originalError = console.error
  beforeEach(() => {
    console.error = vi.fn()
  })
  afterEach(() => {
    console.error = originalError
  })

  it('renders children when no error', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundary>
    )
    expect(screen.getByText('All good')).toBeInTheDocument()
  })

  it('shows fallback UI when child throws', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    )
    expect(screen.getByText('Algo salió mal')).toBeInTheDocument()
    expect(screen.getByText('Test explosion')).toBeInTheDocument()
  })

  it('shows retry button in error state', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    )
    expect(screen.getByText('Reintentar')).toBeInTheDocument()
  })

  it('resets error state on retry click', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    )
    expect(screen.getByText('Algo salió mal')).toBeInTheDocument()

    // Click retry — ErrorBoundary resets state, but the same child will throw again
    fireEvent.click(screen.getByText('Reintentar'))
    // After reset, it tries to render children again, which throws again
    expect(screen.getByText('Algo salió mal')).toBeInTheDocument()
  })

  it('shows default message when error has no message', () => {
    function ThrowEmpty(): ReactElement {
      throw new Error('')
    }
    render(
      <ErrorBoundary>
        <ThrowEmpty />
      </ErrorBoundary>
    )
    expect(screen.getByText('Error inesperado en la aplicación')).toBeInTheDocument()
  })
})
