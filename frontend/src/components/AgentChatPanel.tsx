import type { ReactNode } from 'react'

import { formatTimestamp } from '../lib/format'
import { Button } from './ui'

export type AgentChatMessage = {
  role: 'user' | 'assistant'
  text: string
  timestamp?: string
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
  onClose,
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
  onClose: () => void
  className?: string
}): ReactNode {
  const composerDisabled = loading || inputDisabled
  const sendDisabled = composerDisabled || input.trim().length === 0

  return (
    <div className={['flex min-h-0 flex-col', className].filter(Boolean).join(' ')}>
      <div className="flex shrink-0 items-center justify-between gap-2">
        <h4 className="text-sm font-semibold">Agent Chat{titleSuffix}</h4>
        <Button
          aria-label="關閉 Agent Chat"
          className="p-1.5"
          onClick={onClose}
          title="關閉 Agent Chat"
          variant="ghost"
        >
          <svg aria-hidden="true" className="size-4" fill="currentColor" viewBox="0 0 48 48">
            <path d="M12.4 12.4a2 2 0 0 1 2.8 0L24 21.2l8.8-8.8a2 2 0 1 1 2.8 2.8L26.8 24l8.8 8.8a2 2 0 0 1-2.8 2.8L24 26.8l-8.8 8.8a2 2 0 0 1-2.8-2.8L21.2 24l-8.8-8.8a2 2 0 0 1 0-2.8Z" />
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
                  'flex min-w-0 max-w-[85%] flex-col gap-1',
                  message.role === 'user' ? 'items-end' : 'items-start',
                ].join(' ')}
              >
                <div
                  className={[
                    'min-w-0 max-w-full break-words whitespace-pre-wrap rounded-xl px-3 py-2 text-sm leading-6',
                    message.role === 'user' ? 'bg-mr-soft text-mr-dark' : 'bg-page text-ink-secondary',
                  ].join(' ')}
                >
                  {message.text}
                </div>
                {message.timestamp ? (
                  <span className="px-1 text-xs text-ink-muted">
                    {formatTimestamp(message.timestamp)}
                  </span>
                ) : null}
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
