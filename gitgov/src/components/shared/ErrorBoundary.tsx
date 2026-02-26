import { Component, type ReactNode } from 'react'
import { AlertTriangle, RotateCcw } from 'lucide-react'

interface Props {
  children: ReactNode
}

interface State {
  hasError: boolean
  error: Error | null
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('[ErrorBoundary]', error, info.componentStack)
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null })
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-dvh bg-surface-950 flex flex-col items-center justify-center p-8">
          <div className="max-w-md w-full text-center">
            <div className="w-12 h-12 rounded-xl bg-danger-600/20 flex items-center justify-center mx-auto mb-5">
              <AlertTriangle size={24} className="text-danger-400" />
            </div>
            <h1 className="text-lg font-semibold text-white mb-2">Algo salió mal</h1>
            <p className="text-sm text-surface-400 mb-6">
              {this.state.error?.message || 'Error inesperado en la aplicación'}
            </p>
            <button
              onClick={this.handleReset}
              className="inline-flex items-center gap-2 px-4 py-2 bg-brand-600 hover:bg-brand-500 text-white text-sm rounded-lg transition-colors"
            >
              <RotateCcw size={14} />
              Reintentar
            </button>
          </div>
        </div>
      )
    }

    return this.props.children
  }
}
