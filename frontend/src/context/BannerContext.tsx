import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react'

type BannerState = {
  message: string | null
  isError: boolean
  show: (message: string, isError?: boolean) => void
  dismiss: () => void
}

const BannerContext = createContext<BannerState | null>(null)

export function BannerProvider({ children }: { children: ReactNode }) {
  const [message, setMessage] = useState<string | null>(null)
  const [isError, setIsError] = useState(false)

  const show = useCallback((next: string, error = false) => {
    setMessage(next)
    setIsError(error)
  }, [])

  const dismiss = useCallback(() => {
    setMessage(null)
    setIsError(false)
  }, [])

  const value = useMemo(
    () => ({ message, isError, show, dismiss }),
    [message, isError, show, dismiss],
  )

  return <BannerContext.Provider value={value}>{children}</BannerContext.Provider>
}

export function useBanner(): BannerState {
  const ctx = useContext(BannerContext)
  if (!ctx) {
    throw new Error('useBanner must be used within BannerProvider')
  }
  return ctx
}
