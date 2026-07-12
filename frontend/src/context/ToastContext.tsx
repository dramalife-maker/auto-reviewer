import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'

const SUCCESS_MS = 4000
const ERROR_MS = 8000

type ToastState = {
  message: string | null
  isError: boolean
  show: (message: string, isError?: boolean) => void
  dismiss: () => void
}

const ToastContext = createContext<ToastState | null>(null)

export function ToastProvider({ children }: { children: ReactNode }) {
  const [message, setMessage] = useState<string | null>(null)
  const [isError, setIsError] = useState(false)
  const [token, setToken] = useState(0)

  const dismiss = useCallback(() => {
    setMessage(null)
    setIsError(false)
  }, [])

  const show = useCallback((next: string, error = false) => {
    setMessage(next)
    setIsError(error)
    setToken((value) => value + 1)
  }, [])

  useEffect(() => {
    if (!message) return
    const timer = window.setTimeout(dismiss, isError ? ERROR_MS : SUCCESS_MS)
    return () => window.clearTimeout(timer)
  }, [dismiss, isError, message, token])

  const value = useMemo(
    () => ({ message, isError, show, dismiss }),
    [message, isError, show, dismiss],
  )

  return <ToastContext.Provider value={value}>{children}</ToastContext.Provider>
}

export function useToast(): ToastState {
  const ctx = useContext(ToastContext)
  if (!ctx) {
    throw new Error('useToast must be used within ToastProvider')
  }
  return ctx
}
