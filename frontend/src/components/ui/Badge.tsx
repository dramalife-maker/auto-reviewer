import type { ReactNode } from 'react'

type Tone = 'primary' | 'mr' | 'warning' | 'success' | 'neutral'

const toneClass: Record<Tone, string> = {
  primary: 'bg-primary text-white',
  mr: 'bg-mr text-white',
  warning: 'bg-warning text-white',
  success: 'bg-success text-white',
  neutral: 'bg-page text-ink-muted border border-border',
}

export function Badge({
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
        'inline-flex min-w-[18px] items-center justify-center rounded-full px-1.5 text-[11px] font-semibold leading-[18px]',
        toneClass[tone],
        className,
      ].join(' ')}
    >
      {children}
    </span>
  )
}
