import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ApiError } from '../api'
import { ToastProvider } from '../context/ToastContext.tsx'
import type { MrReviewItem, MrReviewStatus } from '../types'
import { MrInboxPage } from './MrInboxPage.tsx'

const fetchMrReviews = vi.fn<(status?: MrReviewStatus) => Promise<MrReviewItem[]>>()
const updateMrReview = vi.fn()
const agentTurnMrReview = vi.fn()
const restoreMrReview = vi.fn()

vi.mock('../api', async () => {
  const actual = await vi.importActual<typeof import('../api')>('../api')
  return {
    ...actual,
    fetchMrReviews: (status?: MrReviewStatus) => fetchMrReviews(status),
    updateMrReview: (...args: unknown[]) => updateMrReview(...args),
    publishMrReview: vi.fn(),
    ignoreMrReview: vi.fn(),
    restoreMrReview: (...args: unknown[]) => restoreMrReview(...args),
    agentTurnMrReview: (...args: unknown[]) => agentTurnMrReview(...args),
  }
})

function mrReview(
  status: MrReviewStatus,
  id: number,
  overrides: Partial<MrReviewItem> = {},
): MrReviewItem {
  return {
    id,
    project_id: 10,
    project_name: 'game-backend',
    person_id: 1,
    author_name: 'Alice Chen',
    mr_iid: 42,
    mr_title: `${status} MR`,
    review_round: 1,
    status,
    draft_body: `body ${status}`,
    draft_hash: `hash-${status}-${id}`,
    chat_messages: [],
    agent_session_id: status === 'draft' ? 'session-1' : null,
    reviewer_agent: 'cursor',
    created_at: '2026-07-12 06:03:00',
    ...overrides,
  }
}

function renderPage(initialEntry = '/mr-inbox?status=published') {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <ToastProvider>
        <MrInboxPage />
      </ToastProvider>
    </MemoryRouter>,
  )
}

describe('MrInboxPage', () => {
  beforeEach(() => {
    fetchMrReviews.mockReset()
    updateMrReview.mockReset()
    agentTurnMrReview.mockReset()
    restoreMrReview.mockReset()
    fetchMrReviews.mockImplementation(async (status = 'draft') => [mrReview(status, 1)])
  })

  it('syncs the status query parameter with the active filter', async () => {
    const user = userEvent.setup()
    renderPage()

    await waitFor(() => expect(fetchMrReviews).toHaveBeenCalledWith('published'))
    expect(await screen.findByRole('tab', { name: '已發布' })).toHaveAttribute(
      'aria-selected',
      'true',
    )

    await user.click(screen.getByRole('tab', { name: '草稿' }))

    await waitFor(() => expect(fetchMrReviews).toHaveBeenLastCalledWith('draft'))
    expect(screen.getByRole('tab', { name: '草稿' })).toHaveAttribute('aria-selected', 'true')
  })

  it('toggles markdown preview for the selected review', async () => {
    const user = userEvent.setup()
    renderPage('/mr-inbox')

    await screen.findByLabelText('Review 草稿')

    const modeGroup = screen.getByRole('group', { name: '編輯模式' })
    await user.click(within(modeGroup).getByRole('button', { name: 'Preview' }))

    expect(within(modeGroup).getByRole('button', { name: 'Preview' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getByLabelText('Markdown 預覽')).toHaveTextContent('body draft')
    expect(screen.queryByLabelText('Review 草稿')).not.toBeInTheDocument()

    await user.click(within(modeGroup).getByRole('button', { name: '編輯' }))

    expect(screen.getByLabelText('Review 草稿')).toBeInTheDocument()
  })

  it('hydrates Agent Chat from list payload chat_messages', async () => {
    fetchMrReviews.mockResolvedValue([
      mrReview('draft', 1, {
        chat_messages: [
          {
            id: 1,
            role: 'user',
            content: 'why flag helper?',
            created_at: '2026-07-13 01:00:00',
          },
          {
            id: 2,
            role: 'assistant',
            content: 'because it wraps commits',
            created_at: '2026-07-13 01:00:01',
          },
        ],
      }),
    ])
    renderPage('/mr-inbox')

    expect(await screen.findByText('why flag helper?')).toBeInTheDocument()
    expect(screen.getByText('because it wraps commits')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '送出' })).toBeInTheDocument()
  })

  it('shows published chat history as read-only without send controls', async () => {
    fetchMrReviews.mockResolvedValue([
      mrReview('published', 1, {
        chat_messages: [
          {
            id: 1,
            role: 'user',
            content: 'why flag helper?',
            created_at: '2026-07-13 01:00:00',
          },
          {
            id: 2,
            role: 'assistant',
            content: 'because it wraps commits',
            created_at: '2026-07-13 01:00:01',
          },
        ],
      }),
    ])
    renderPage('/mr-inbox?status=published')

    expect(await screen.findByText('Agent Chat（唯讀）')).toBeInTheDocument()
    expect(screen.getByText('why flag helper?')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '送出' })).not.toBeInTheDocument()
  })

  it('hides Agent Chat for published reviews without history', async () => {
    fetchMrReviews.mockResolvedValue([mrReview('published', 1)])
    renderPage('/mr-inbox?status=published')

    await screen.findByText('Review 內容（唯讀）')
    expect(screen.queryByText(/Agent Chat/)).not.toBeInTheDocument()
  })

  it('restores an ignored review back to draft', async () => {
    const user = userEvent.setup()
    fetchMrReviews.mockImplementation(async (status = 'draft') => {
      if (status === 'ignored') {
        return [mrReview('ignored', 5, { mr_iid: 5, mr_title: 'ignored MR' })]
      }
      return [mrReview('draft', 5, { mr_iid: 5, mr_title: 'ignored MR', status: 'draft' })]
    })
    restoreMrReview.mockResolvedValue(undefined)

    renderPage('/mr-inbox?status=ignored')
    expect(await screen.findByRole('button', { name: '復原為草稿' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '復原為草稿' }))

    await waitFor(() => expect(restoreMrReview).toHaveBeenCalledWith(5))
    await waitFor(() => expect(fetchMrReviews).toHaveBeenCalledWith('draft'))
  })

  it('adopts agent draft and shows new-version marker when editor is clean', async () => {
    const user = userEvent.setup()
    agentTurnMrReview.mockResolvedValue({
      reply: 'updated',
      agent_session_id: 'session-2',
      draft_body: 'agent new body',
      draft_hash: 'hash-new',
    })
    renderPage('/mr-inbox')

    const input = await screen.findByPlaceholderText('例如：為什麼你標記了 transaction helper？')
    await user.type(input, 'please revise')
    await user.click(screen.getByRole('button', { name: '送出' }))

    await waitFor(() => expect(agentTurnMrReview).toHaveBeenCalled())
    expect(await screen.findByText('草稿有新版本')).toBeInTheDocument()
    expect(screen.getByLabelText('Review 草稿')).toHaveValue('agent new body')
  })

  it('shows conflict choices when agent returns new draft while editor is dirty', async () => {
    const user = userEvent.setup()
    agentTurnMrReview.mockResolvedValue({
      reply: 'updated',
      agent_session_id: 'session-2',
      draft_body: 'agent new body',
      draft_hash: 'hash-new',
    })
    renderPage('/mr-inbox')

    const editor = await screen.findByLabelText('Review 草稿')
    fireEvent.change(editor, { target: { value: 'my local edit' } })

    const input = screen.getByPlaceholderText('例如：為什麼你標記了 transaction helper？')
    await user.type(input, 'please revise')
    await user.click(screen.getByRole('button', { name: '送出' }))

    expect(await screen.findByRole('button', { name: '預覽新版本' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '載入新版本' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保留我的編輯' })).toBeInTheDocument()
    expect(screen.getByLabelText('Review 草稿')).toHaveValue('my local edit')
  })

  it('preview new version is read-only and keeps editor text', async () => {
    const user = userEvent.setup()
    agentTurnMrReview.mockResolvedValue({
      reply: 'updated',
      agent_session_id: 'session-2',
      draft_body: 'agent new body',
      draft_hash: 'hash-new',
    })
    renderPage('/mr-inbox')

    const editor = await screen.findByLabelText('Review 草稿')
    fireEvent.change(editor, { target: { value: 'my local edit' } })

    const input = screen.getByPlaceholderText('例如：為什麼你標記了 transaction helper？')
    await user.type(input, 'please revise')
    await user.click(screen.getByRole('button', { name: '送出' }))

    await user.click(await screen.findByRole('button', { name: '預覽新版本' }))

    expect(screen.getByLabelText('衝突草稿預覽')).toHaveTextContent('agent new body')
    await user.click(screen.getByRole('button', { name: '保留我的編輯' }))
    expect(screen.getByLabelText('Review 草稿')).toHaveValue('my local edit')
  })

  it('save sends base_hash and surfaces conflict choices on 409', async () => {
    const user = userEvent.setup()
    updateMrReview.mockRejectedValue(
      new ApiError(
        JSON.stringify({ draft_body: 'server body', draft_hash: 'hash-server' }),
        409,
        JSON.stringify({ draft_body: 'server body', draft_hash: 'hash-server' }),
      ),
    )
    renderPage('/mr-inbox')

    const editor = await screen.findByLabelText('Review 草稿')
    fireEvent.change(editor, { target: { value: 'local save' } })
    await user.click(screen.getByRole('button', { name: '儲存草稿' }))

    await waitFor(() =>
      expect(updateMrReview).toHaveBeenCalledWith(1, 'local save', 'hash-draft-1'),
    )
    expect(await screen.findByRole('button', { name: '預覽新版本' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '載入新版本' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保留我的編輯' })).toBeInTheDocument()
  })
})
