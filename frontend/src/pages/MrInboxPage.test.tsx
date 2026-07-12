import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ToastProvider } from '../context/ToastContext.tsx'
import type { MrReviewItem, MrReviewStatus } from '../types'
import { MrInboxPage } from './MrInboxPage.tsx'

const fetchMrReviews = vi.fn<(status?: MrReviewStatus) => Promise<MrReviewItem[]>>()

vi.mock('../api', () => ({
  fetchMrReviews: (status?: MrReviewStatus) => fetchMrReviews(status),
  updateMrReview: vi.fn(),
  publishMrReview: vi.fn(),
  ignoreMrReview: vi.fn(),
  agentTurnMrReview: vi.fn(),
}))

function mrReview(status: MrReviewStatus, id: number): MrReviewItem {
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
    agent_session_id: status === 'draft' ? 'session-1' : null,
    reviewer_agent: 'cursor',
    created_at: '2026-07-12 06:03:00',
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
})
