import { render, screen, fireEvent } from '@testing-library/react'
import { DashboardHeader } from '@/components/control_plane/DashboardHeader'

describe('DashboardHeader', () => {
  const defaultProps = {
    autoRefresh: false,
    onAutoRefreshChange: vi.fn(),
    onRefresh: vi.fn(),
    isRefreshing: false,
  }

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders title', () => {
    render(<DashboardHeader {...defaultProps} />)
    expect(screen.getByText('Dashboard')).toBeInTheDocument()
  })

  it('renders subtitle', () => {
    render(<DashboardHeader {...defaultProps} />)
    expect(screen.getByText('Control Plane overview')).toBeInTheDocument()
  })

  it('renders auto-refresh checkbox', () => {
    render(<DashboardHeader {...defaultProps} />)
    expect(screen.getByLabelText('Auto-refresh')).toBeInTheDocument()
  })

  it('checkbox reflects autoRefresh prop', () => {
    const { rerender } = render(<DashboardHeader {...defaultProps} autoRefresh={false} />)
    expect(screen.getByLabelText('Auto-refresh')).not.toBeChecked()

    rerender(<DashboardHeader {...defaultProps} autoRefresh={true} />)
    expect(screen.getByLabelText('Auto-refresh')).toBeChecked()
  })

  it('calls onAutoRefreshChange when checkbox toggled', () => {
    const onChange = vi.fn()
    render(<DashboardHeader {...defaultProps} onAutoRefreshChange={onChange} />)
    fireEvent.click(screen.getByLabelText('Auto-refresh'))
    expect(onChange).toHaveBeenCalledWith(true)
  })

  it('renders refresh button', () => {
    render(<DashboardHeader {...defaultProps} />)
    expect(screen.getByText('Actualizar')).toBeInTheDocument()
  })

  it('calls onRefresh when button clicked', () => {
    const onRefresh = vi.fn()
    render(<DashboardHeader {...defaultProps} onRefresh={onRefresh} />)
    fireEvent.click(screen.getByText('Actualizar'))
    expect(onRefresh).toHaveBeenCalledTimes(1)
  })

  it('shows loading state on refresh button', () => {
    render(<DashboardHeader {...defaultProps} isRefreshing={true} />)
    // Button should be disabled when isRefreshing (loading prop)
    expect(screen.getByRole('button', { name: /Actualizar/i })).toBeDisabled()
  })
})
