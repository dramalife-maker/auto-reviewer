import type { ButtonHTMLAttributes, ReactNode } from 'react'

export function ListRow({
  active = false,
  accent = 'primary',
  children,
  className = '',
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean
  accent?: 'primary' | 'mr'
  children: ReactNode
}) {
  const inset =
    accent === 'mr' ? 'shadow-[inset_3px_0_0_#7c3aed]' : 'shadow-[inset_3px_0_0_#4f46e5]'
  const activeBg = accent === 'mr' ? 'bg-mr-tint' : 'bg-primary-tint'

  return (
    <button
      type="button"
      className={[
        'block w-full px-3 py-2.5 text-left text-[13.5px]',
        active ? `${activeBg} ${inset}` : 'bg-transparent hover:bg-page',
        className,
      ].join(' ')}
      {...rest}
    >
      {children}
    </button>
  )
}
