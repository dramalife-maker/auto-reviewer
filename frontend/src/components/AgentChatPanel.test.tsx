import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { AgentChatPanel } from './AgentChatPanel'

const baseProps = {
  messages: [] as Array<{ role: 'user' | 'assistant'; text: string }>,
  input: '',
  loading: false,
  emptyHint: '針對這份 review 向 AI 追問細節。',
  placeholder: '例如：為什麼你標記了 transaction helper？',
  onInputChange: vi.fn(),
  onSend: vi.fn(),
  onCollapse: vi.fn(),
}

describe('AgentChatPanel', () => {
  it('shows empty hint and hides send when readOnly', () => {
    render(
      <AgentChatPanel
        {...baseProps}
        messages={[
          { role: 'user', text: 'why flag helper?' },
          { role: 'assistant', text: 'because it wraps commits' },
        ]}
        emptyHint="unused when messages exist"
        readOnly
        titleSuffix="（唯讀）"
      />,
    )

    expect(screen.getByText('Agent Chat（唯讀）')).toBeInTheDocument()
    expect(screen.getByText('why flag helper?')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '送出' })).not.toBeInTheDocument()
  })

  it('disables composer when inputDisabled', () => {
    render(
      <AgentChatPanel
        {...baseProps}
        input="hello"
        inputDisabled
        placeholder="此草稿沒有 agent session"
      />,
    )

    expect(screen.getByPlaceholderText('此草稿沒有 agent session')).toBeDisabled()
    expect(screen.getByRole('button', { name: '送出' })).toBeDisabled()
  })

  it('shows emptyHint when there are no messages', () => {
    render(<AgentChatPanel {...baseProps} emptyHint="討論並調整這位人員的週報／觀察檔。" />)

    expect(screen.getByText('討論並調整這位人員的週報／觀察檔。')).toBeInTheDocument()
  })

  it('calls onSend when send is clicked', async () => {
    const user = userEvent.setup()
    const onSend = vi.fn()
    render(<AgentChatPanel {...baseProps} input="please revise" onSend={onSend} />)

    await user.click(screen.getByRole('button', { name: '送出' }))
    expect(onSend).toHaveBeenCalledTimes(1)
  })
})
