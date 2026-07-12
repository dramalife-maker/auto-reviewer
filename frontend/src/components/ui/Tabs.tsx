export function Tabs({
  items,
  value,
  onChange,
  accent = 'primary',
}: {
  items: { id: string; label: string }[]
  value: string
  onChange: (id: string) => void
  accent?: 'primary' | 'mr'
}) {
  const underline =
    accent === 'mr' ? 'shadow-[inset_0_-2px_0_#7c3aed]' : 'shadow-[inset_0_-2px_0_#4f46e5]'
  const activeText = accent === 'mr' ? 'text-mr-dark' : 'text-primary'

  return (
    <div className="flex gap-1 border-b border-border" role="tablist">
      {items.map((item) => {
        const active = item.id === value
        return (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(item.id)}
            className={[
              'px-3 py-2 text-[13px] font-medium',
              active ? `${activeText} ${underline}` : 'text-ink-muted',
            ].join(' ')}
          >
            {item.label}
          </button>
        )
      })}
    </div>
  )
}
