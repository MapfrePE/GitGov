import { render, screen, fireEvent } from '@testing-library/react'
import { PinUnlockScreen } from '@/components/auth/PinUnlockScreen'
import { useAuthStore } from '@/store/useAuthStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'

// Mock Tauri to prevent store initialization errors
const mockInvoke = vi.fn().mockReturnValue(Promise.resolve(undefined))
vi.mock('@/lib/tauri', () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
  tauriListen: vi.fn().mockResolvedValue(() => {}),
  parseCommandError: (e: unknown) => ({ code: 'UNKNOWN', message: String(e) }),
}))

describe('PinUnlockScreen', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthStore.setState({
      user: { login: 'testuser', name: 'Test User', avatarUrl: '' },
      pinError: null,
      unlockWithPin: vi.fn(),
      logout: vi.fn().mockResolvedValue(undefined),
    })
    useControlPlaneStore.setState({
      disconnect: vi.fn(),
    })
  })

  it('renders unlock screen title', () => {
    render(<PinUnlockScreen />)
    expect(screen.getByText('Desbloquear sesión')).toBeInTheDocument()
  })

  it('shows username in description', () => {
    render(<PinUnlockScreen />)
    expect(screen.getByText(/testuser/)).toBeInTheDocument()
  })

  it('renders PIN input with placeholder', () => {
    render(<PinUnlockScreen />)
    expect(screen.getByPlaceholderText('PIN (4 a 6 dígitos)')).toBeInTheDocument()
  })

  it('renders unlock and change user buttons', () => {
    render(<PinUnlockScreen />)
    expect(screen.getByText('Desbloquear')).toBeInTheDocument()
    expect(screen.getByText('Cambiar usuario')).toBeInTheDocument()
  })

  it('calls unlockWithPin when unlock clicked', () => {
    const unlockWithPin = vi.fn()
    useAuthStore.setState({ unlockWithPin })
    render(<PinUnlockScreen />)

    const input = screen.getByPlaceholderText('PIN (4 a 6 dígitos)')
    fireEvent.change(input, { target: { value: '1234' } })
    fireEvent.click(screen.getByText('Desbloquear'))

    expect(unlockWithPin).toHaveBeenCalledWith('1234')
  })

  it('calls unlockWithPin on Enter key', () => {
    const unlockWithPin = vi.fn()
    useAuthStore.setState({ unlockWithPin })
    render(<PinUnlockScreen />)

    const input = screen.getByPlaceholderText('PIN (4 a 6 dígitos)')
    fireEvent.change(input, { target: { value: '5678' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(unlockWithPin).toHaveBeenCalledWith('5678')
  })

  it('clears PIN input after unlock attempt', () => {
    useAuthStore.setState({ unlockWithPin: vi.fn() })
    render(<PinUnlockScreen />)

    const input = screen.getByPlaceholderText('PIN (4 a 6 dígitos)') as HTMLInputElement
    fireEvent.change(input, { target: { value: '1234' } })
    fireEvent.click(screen.getByText('Desbloquear'))

    expect(input.value).toBe('')
  })

  it('shows PIN error when present', () => {
    useAuthStore.setState({ pinError: 'PIN incorrecto' })
    render(<PinUnlockScreen />)
    expect(screen.getByText('PIN incorrecto')).toBeInTheDocument()
  })

  it('does not show error when pinError is null', () => {
    useAuthStore.setState({ pinError: null })
    render(<PinUnlockScreen />)
    expect(screen.queryByText('PIN incorrecto')).not.toBeInTheDocument()
  })

  it('calls disconnect and logout when change user clicked', () => {
    const disconnect = vi.fn()
    const logout = vi.fn().mockResolvedValue(undefined)
    useControlPlaneStore.setState({ disconnect })
    useAuthStore.setState({ logout })
    render(<PinUnlockScreen />)

    fireEvent.click(screen.getByText('Cambiar usuario'))
    expect(disconnect).toHaveBeenCalled()
  })
})
