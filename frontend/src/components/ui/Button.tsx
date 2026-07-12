import type { ButtonHTMLAttributes, ReactNode } from 'react'

type Variant = 'primary' | 'secondary' | 'danger' | 'mr' | 'ghost'

const variantClass: Record<Variant, string> = {
  primary: 'bg-primary text-white hover:bg-primary-dark',
  secondary: 'bg-surface text-ink-secondary border border-border hover:bg-page',
  danger: 'bg-surface text-danger border border-border hover:bg-danger-tint',
  mr: 'bg-mr text-white hover:bg-mr-dark',
  ghost: 'bg-transparent text-ink-secondary hover:bg-page',
}

export function Button({
  variant = 'secondary',
  className = '',
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant
  children: ReactNode
}) {
  return (
    <button
      type="button"
      className={[
        'inline-flex items-center justify-center rounded-md px-[18px] py-2.5 text-[13.5px] font-semibold disabled:opacity-50',
        variantClass[variant],
        className,
      ].join(' ')}
      {...rest}
    >
      {children}
    </button>
  )
}
