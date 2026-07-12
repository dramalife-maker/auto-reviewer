import { useToast } from '../../context/ToastContext.tsx'

export function Toast() {
  const { message, isError, dismiss } = useToast()
  if (!message) return null

  return (
    <div
      role="status"
      className={[
        'fixed right-6 top-6 z-50 flex max-w-md items-start gap-3 rounded-lg border px-4 py-3 text-[13.5px] shadow-lg',
        isError
          ? 'border-danger-border bg-danger-tint text-danger'
          : 'border-border bg-primary-tint text-primary-dark',
      ].join(' ')}
    >
      <span className="min-w-0 flex-1 leading-snug">{message}</span>
      <button
        type="button"
        aria-label="關閉提示"
        className="shrink-0 text-lg leading-none text-ink-meta hover:text-ink"
        onClick={dismiss}
      >
        ×
      </button>
    </div>
  )
}
