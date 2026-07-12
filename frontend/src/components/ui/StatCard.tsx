import type { ReactNode } from 'react'

export function StatCard({
  label,
  value,
  valueClassName = 'text-ink',
  onClick,
}: {
  label: string
  value: ReactNode
  valueClassName?: string
  onClick?: () => void
}) {
  const body = (
    <>
      <div className="text-xs text-ink-muted">{label}</div>
      <div className={['mt-1 text-[26px] font-semibold', valueClassName].join(' ')}>{value}</div>
    </>
  )

  if (onClick) {
    return (
      <button
        type="button"
        onClick={onClick}
        className="rounded-xl border border-border bg-surface p-4 text-left"
      >
        {body}
      </button>
    )
  }

  return <div className="rounded-xl border border-border bg-surface p-4">{body}</div>
}
