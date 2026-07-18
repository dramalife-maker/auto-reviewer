import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { AgentChatLauncher } from './AgentChatLauncher'

const panelProps = {
  messages: [] as Array<{ role: 'user' | 'assistant'; text: string }>,
  input: '',
  loading: false,
  emptyHint: '討論並調整這位人員的週報／觀察檔。',
  placeholder: '例如：把 alpha 的 one_line 改得更精準',
  onInputChange: vi.fn(),
  onSend: vi.fn(),
}

describe('AgentChatLauncher', () => {
  it('defaults to closed with FAB only', () => {
    render(<AgentChatLauncher {...panelProps} />)

    expect(screen.getByRole('button', { name: '展開 Agent Chat' })).toBeInTheDocument()
    expect(screen.queryByText('Agent Chat')).not.toBeInTheDocument()
    expect(screen.queryByText('討論並調整這位人員的週報／觀察檔。')).not.toBeInTheDocument()
  })

  it('opens overlay from FAB', async () => {
    const user = userEvent.setup()
    render(<AgentChatLauncher {...panelProps} />)

    await user.click(screen.getByRole('button', { name: '展開 Agent Chat' }))

    expect(screen.getByText('Agent Chat')).toBeInTheDocument()
    expect(screen.getByText('討論並調整這位人員的週報／觀察檔。')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '展開 Agent Chat' })).not.toBeInTheDocument()
  })

  it('closes overlay and returns to FAB', async () => {
    const user = userEvent.setup()
    render(<AgentChatLauncher {...panelProps} />)

    await user.click(screen.getByRole('button', { name: '展開 Agent Chat' }))
    await user.click(screen.getByRole('button', { name: '關閉 Agent Chat' }))

    expect(screen.getByRole('button', { name: '展開 Agent Chat' })).toBeInTheDocument()
    expect(screen.queryByText('討論並調整這位人員的週報／觀察檔。')).not.toBeInTheDocument()
  })
})
