import type { ReactNode } from 'react'

type Tone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary' | 'mr'

const toneClass: Record<Tone, string> = {
  success: 'bg-success-tint text-success border-success-border',
  warning: 'bg-warning-tint text-warning-ink border-warning-border',
  danger: 'bg-danger-tint text-danger border-danger-border',
  neutral: 'bg-page text-ink-muted border-border',
  primary: 'bg-primary-tint text-primary border-primary/20',
  mr: 'bg-mr-tint text-mr-dark border-mr/20',
}

export function StatusPill({
  tone = 'neutral',
  children,
  className = '',
}: {
  tone?: Tone
  children: ReactNode
  className?: string
}) {
  return (
    <span
      className={[
        'inline-flex items-center rounded-full border px-2 py-0.5 text-[11.5px] font-medium',
        toneClass[tone],
        className,
      ].join(' ')}
    >
      {children}
    </span>
  )
}
