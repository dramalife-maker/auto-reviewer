import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

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

beforeEach(() => {
  window.localStorage.clear()
})

afterEach(() => {
  window.localStorage.clear()
})

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

  it('drags the FAB to a new position and persists it', () => {
    render(<AgentChatLauncher {...panelProps} />)
    const fab = screen.getByRole('button', { name: '展開 Agent Chat' })

    fireEvent.pointerDown(fab, { clientX: 100, clientY: 100 })
    fireEvent.pointerMove(fab, { clientX: 40, clientY: 60 })
    fireEvent.pointerUp(fab, { clientX: 40, clientY: 60 })

    expect(fab.style.right).not.toBe('')
    expect(fab.style.bottom).not.toBe('')
    expect(window.localStorage.getItem('agent-chat-fab-position')).not.toBeNull()
  })

  it('does not open the overlay when the FAB drag click follows a real drag', () => {
    render(<AgentChatLauncher {...panelProps} />)
    const fab = screen.getByRole('button', { name: '展開 Agent Chat' })

    fireEvent.pointerDown(fab, { clientX: 100, clientY: 100 })
    fireEvent.pointerMove(fab, { clientX: 40, clientY: 60 })
    fireEvent.pointerUp(fab, { clientX: 40, clientY: 60 })
    fireEvent.click(fab)

    expect(screen.queryByText('Agent Chat')).not.toBeInTheDocument()
  })

  it('opens the overlay on a plain click without movement', () => {
    render(<AgentChatLauncher {...panelProps} />)
    const fab = screen.getByRole('button', { name: '展開 Agent Chat' })

    fireEvent.pointerDown(fab, { clientX: 100, clientY: 100 })
    fireEvent.pointerUp(fab, { clientX: 100, clientY: 100 })
    fireEvent.click(fab)

    expect(screen.getByText('Agent Chat')).toBeInTheDocument()
  })

  it('drags the expanded panel via its header', async () => {
    const user = userEvent.setup()
    render(<AgentChatLauncher {...panelProps} />)

    await user.click(screen.getByRole('button', { name: '展開 Agent Chat' }))
    const header = screen.getByText('Agent Chat').parentElement as HTMLElement

    fireEvent.pointerDown(header, { clientX: 200, clientY: 200 })
    fireEvent.pointerMove(header, { clientX: 150, clientY: 130 })
    fireEvent.pointerUp(header, { clientX: 150, clientY: 130 })

    expect(window.localStorage.getItem('agent-chat-panel-position')).not.toBeNull()
  })
})
