import { forwardRef, type CSSProperties, type ReactNode } from 'react'

export const Card = forwardRef<
  HTMLDivElement,
  {
    children: ReactNode
    className?: string
    style?: CSSProperties
  }
>(function Card({ children, className = '', style }, ref) {
  return (
    <div ref={ref} className={['rounded-xl border border-border bg-surface', className].join(' ')} style={style}>
      {children}
    </div>
  )
})
