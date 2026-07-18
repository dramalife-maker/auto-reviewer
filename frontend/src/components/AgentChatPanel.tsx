import type { ReactNode } from 'react'

import { Button } from './ui'

export type AgentChatMessage = {
  role: 'user' | 'assistant'
  text: string
}

export function AgentChatPanel({
  messages,
  input,
  loading,
  readOnly = false,
  inputDisabled = false,
  emptyHint,
  placeholder,
  titleSuffix = '',
  onInputChange,
  onSend,
  onCollapse,
  className = '',
}: {
  messages: AgentChatMessage[]
  input: string
  loading: boolean
  readOnly?: boolean
  inputDisabled?: boolean
  emptyHint: string
  placeholder: string
  titleSuffix?: string
  onInputChange: (value: string) => void
  onSend: () => void
  onCollapse: () => void
  className?: string
}): ReactNode {
  const composerDisabled = loading || inputDisabled
  const sendDisabled = composerDisabled || input.trim().length === 0

  return (
    <div className={['flex min-h-0 flex-col', className].filter(Boolean).join(' ')}>
      <div className="flex shrink-0 items-center justify-between gap-2">
        <h4 className="text-sm font-semibold">Agent Chat{titleSuffix}</h4>
        <Button
          aria-label="收合 Agent Chat"
          className="p-1.5"
          onClick={onCollapse}
          title="收合 Agent Chat"
          variant="ghost"
        >
          <svg aria-hidden="true" className="size-4" fill="currentColor" viewBox="0 0 48 48">
            <path d="M32.6,22.6a1.9,1.9,0,0,0,0,2.8l5.9,6a2.1,2.1,0,0,0,2.7.2,1.9,1.9,0,0,0,.2-3L38.8,26H44a2,2,0,0,0,0-4H38.8l2.6-2.6a1.9,1.9,0,0,0-.2-3,2.1,2.1,0,0,0-2.7.2Z" />
            <path d="M15.4,25.4a1.9,1.9,0,0,0,0-2.8l-5.9-6a2.1,2.1,0,0,0-2.7-.2,1.9,1.9,0,0,0-.2,3L9.2,22H4a2,2,0,0,0,0,4H9.2L6.6,28.6a1.9,1.9,0,0,0,.2,3,2.1,2.1,0,0,0,2.7-.2Z" />
            <path d="M26,6V42a2,2,0,0,0,4,0V6a2,2,0,0,0-4,0Z" />
            <path d="M22,42V6a2,2,0,0,0-4,0V42a2,2,0,0,0,4,0Z" />
          </svg>
        </Button>
      </div>
      <div className="mt-3 min-h-0 flex-1 space-y-3 overflow-y-auto rounded-lg bg-surface">
        {messages.length === 0 ? (
          <p className="rounded-lg bg-page p-3 text-sm text-ink-muted">{emptyHint}</p>
        ) : (
          messages.map((message, index) => (
            <div
              key={index}
              className={['flex', message.role === 'user' ? 'justify-end' : 'justify-start'].join(
                ' ',
              )}
            >
              <div
                className={[
                  'min-w-0 max-w-[85%] break-words whitespace-pre-wrap rounded-xl px-3 py-2 text-sm leading-6',
                  message.role === 'user' ? 'bg-mr-soft text-mr-dark' : 'bg-page text-ink-secondary',
                ].join(' ')}
              >
                {message.text}
              </div>
            </div>
          ))
        )}
        {loading ? <p className="text-sm text-ink-muted">AI 回覆中...</p> : null}
      </div>
      {!readOnly ? (
        <div className="mt-3 flex shrink-0 gap-2">
          <textarea
            className="max-h-28 min-h-[44px] flex-1 resize-y overflow-y-auto rounded-lg border border-border bg-surface p-2 text-sm outline-none focus:border-mr"
            disabled={composerDisabled}
            onChange={(event) => onInputChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault()
                onSend()
              }
            }}
            placeholder={placeholder}
            value={input}
          />
          <Button disabled={sendDisabled} onClick={onSend} variant="mr">
            送出
          </Button>
        </div>
      ) : null}
    </div>
  )
}
