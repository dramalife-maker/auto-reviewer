import { useEffect, useId, useRef } from 'react'

import { Button } from './Button.tsx'

export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = '確定',
  cancelLabel = '取消',
  confirmVariant = 'primary',
  onConfirm,
  onCancel,
}: {
  open: boolean
  title?: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  confirmVariant?: 'primary' | 'danger' | 'mr'
  onConfirm: () => void
  onCancel: () => void
}) {
  const titleId = useId()
  const messageId = useId()
  const cancelRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!open) return
    cancelRef.current?.focus()

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        onCancel()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [open, onCancel])

  if (!open) return null

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-4"
      role="presentation"
      onClick={onCancel}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={title ? titleId : undefined}
        aria-describedby={messageId}
        className="w-full max-w-md rounded-xl border border-border bg-surface p-5 shadow-lg"
        onClick={(event) => event.stopPropagation()}
      >
        {title ? (
          <h3 id={titleId} className="text-base font-bold text-ink">
            {title}
          </h3>
        ) : null}
        <p id={messageId} className={title ? 'mt-2 text-sm text-ink-secondary' : 'text-sm text-ink-secondary'}>
          {message}
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <Button ref={cancelRef} onClick={onCancel} variant="secondary">
            {cancelLabel}
          </Button>
          <Button onClick={onConfirm} variant={confirmVariant}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  )
}
