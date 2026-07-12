import type { ButtonHTMLAttributes, ReactNode } from 'react'

export function NavItem({
  active = false,
  children,
  trailing,
  className = '',
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean
  trailing?: ReactNode
}) {
  return (
    <button
      type="button"
      className={[
        'flex w-full items-center justify-between rounded-md px-3 py-[9px] text-left text-[13.5px]',
        active
          ? 'bg-primary-tint font-semibold text-primary'
          : 'bg-transparent font-medium text-ink-secondary',
        className,
      ].join(' ')}
      {...rest}
    >
      <span>{children}</span>
      {trailing}
    </button>
  )
}
