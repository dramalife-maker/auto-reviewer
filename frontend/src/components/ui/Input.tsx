import type { InputHTMLAttributes } from 'react'

export function Input({ className = '', ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={[
        'w-full rounded-md border border-border bg-surface px-3 py-2 text-[13.5px] text-ink outline-none focus:border-primary',
        className,
      ].join(' ')}
      {...rest}
    />
  )
}
