import { render, screen, fireEvent } from '@testing-library/react'
import { MetricsGrid } from '@/components/control_plane/MetricsGrid'

describe('MetricsGrid', () => {
  const defaultProps = {
    totalGithubEvents: 1234,
    successRate: '98.5',
    activeRepos: 12,
    desktopPushesToday: 45,
    githubPushesToday: 30,
    totalTrackedPushesToday: 75,
    blockedToday: 3,
    activeDevsWeek: 8,
  }

  it('renders total github events', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('1234')).toBeInTheDocument()
  })

  it('renders success rate with percent', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('98.5%')).toBeInTheDocument()
  })

  it('renders active repos count', () => {
    render(<MetricsGrid {...defaultProps} />)
    // active repos appears in both the hero card and the repos card
    const repoTexts = screen.getAllByText('12')
    expect(repoTexts.length).toBeGreaterThanOrEqual(1)
  })

  it('renders desktop pushes today', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('45')).toBeInTheDocument()
  })

  it('renders github pushes today', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('30')).toBeInTheDocument()
  })

  it('renders total tracked pushes', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('75')).toBeInTheDocument()
  })

  it('renders blocked count', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('3')).toBeInTheDocument()
  })

  it('renders active devs count', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('8')).toBeInTheDocument()
  })

  it('renders section headers', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.getByText('Total Eventos GitHub')).toBeInTheDocument()
    expect(screen.getByText('Pushes Hoy')).toBeInTheDocument()
    expect(screen.getByText('Bloqueados')).toBeInTheDocument()
    expect(screen.getByText('Devs Activos 7d')).toBeInTheDocument()
  })

  it('shows "Ver detalle" link when onOpenActiveDevs provided', () => {
    const onOpen = vi.fn()
    render(<MetricsGrid {...defaultProps} onOpenActiveDevs={onOpen} />)
    expect(screen.getByText('Ver detalle')).toBeInTheDocument()
  })

  it('does not show "Ver detalle" when onOpenActiveDevs not provided', () => {
    render(<MetricsGrid {...defaultProps} />)
    expect(screen.queryByText('Ver detalle')).not.toBeInTheDocument()
  })

  it('calls onOpenActiveDevs when link clicked', () => {
    const onOpen = vi.fn()
    render(<MetricsGrid {...defaultProps} onOpenActiveDevs={onOpen} />)
    fireEvent.click(screen.getByText('Ver detalle'))
    expect(onOpen).toHaveBeenCalledTimes(1)
  })

  it('renders with zero values', () => {
    render(
      <MetricsGrid
        totalGithubEvents={0}
        successRate="0"
        activeRepos={0}
        desktopPushesToday={0}
        githubPushesToday={0}
        totalTrackedPushesToday={0}
        blockedToday={0}
        activeDevsWeek={0}
      />
    )
    // Should render without errors
    expect(screen.getByText('Total Eventos GitHub')).toBeInTheDocument()
  })
})
