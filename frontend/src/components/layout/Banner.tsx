import { useBanner } from '../../context/BannerContext.tsx'

export function Banner() {
  const { message, isError, dismiss } = useBanner()
  if (!message) return null

  return (
    <div
      role="status"
      className={[
        'flex items-start justify-between gap-3 border-b px-10 py-3 text-[13.5px]',
        isError
          ? 'border-danger-border bg-danger-tint text-danger'
          : 'border-border bg-primary-tint text-primary-dark',
      ].join(' ')}
    >
      <span>{message}</span>
      <button
        type="button"
        aria-label="關閉提示"
        className="text-lg leading-none text-ink-meta hover:text-ink"
        onClick={dismiss}
      >
        ×
      </button>
    </div>
  )
}
