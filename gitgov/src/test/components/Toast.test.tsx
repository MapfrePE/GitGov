import { render, screen, fireEvent, act } from '@testing-library/react'
import { ToastContainer, useToastStore, toast } from '@/components/shared/Toast'

describe('Toast system', () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] })
  })

  it('renders no toasts initially', () => {
    render(<ToastContainer />)
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('addToast adds a toast to the store', () => {
    act(() => {
      useToastStore.getState().addToast('success', 'Saved!')
    })
    expect(useToastStore.getState().toasts).toHaveLength(1)
    expect(useToastStore.getState().toasts[0].message).toBe('Saved!')
    expect(useToastStore.getState().toasts[0].type).toBe('success')
  })

  it('removeToast removes a toast from the store', () => {
    act(() => {
      useToastStore.getState().addToast('info', 'Hi')
    })
    const id = useToastStore.getState().toasts[0].id
    act(() => {
      useToastStore.getState().removeToast(id)
    })
    expect(useToastStore.getState().toasts).toHaveLength(0)
  })

  it('toast() helper adds a toast', () => {
    act(() => {
      toast('error', 'Something failed')
    })
    expect(useToastStore.getState().toasts).toHaveLength(1)
    expect(useToastStore.getState().toasts[0].type).toBe('error')
  })

  it('renders toast message in the container', () => {
    act(() => {
      useToastStore.getState().addToast('warning', 'Watch out!')
    })
    render(<ToastContainer />)
    expect(screen.getByText('Watch out!')).toBeInTheDocument()
  })

  it('renders close button for each toast', () => {
    act(() => {
      useToastStore.getState().addToast('info', 'Msg 1')
      useToastStore.getState().addToast('success', 'Msg 2')
    })
    render(<ToastContainer />)
    // Each toast has an X close button
    const buttons = screen.getAllByRole('button')
    expect(buttons.length).toBe(2)
  })

  it('clicking close removes the toast', () => {
    act(() => {
      useToastStore.getState().addToast('info', 'Removable')
    })
    render(<ToastContainer />)
    fireEvent.click(screen.getByRole('button'))
    expect(useToastStore.getState().toasts).toHaveLength(0)
  })

  it('multiple toasts render independently', () => {
    act(() => {
      useToastStore.getState().addToast('success', 'First')
      useToastStore.getState().addToast('error', 'Second')
      useToastStore.getState().addToast('warning', 'Third')
    })
    render(<ToastContainer />)
    expect(screen.getByText('First')).toBeInTheDocument()
    expect(screen.getByText('Second')).toBeInTheDocument()
    expect(screen.getByText('Third')).toBeInTheDocument()
  })
})
