import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ToastProvider } from '../context/ToastContext.tsx'
import type { LatestReportsResponse, Person, PersonReportChatResponse } from '../types'
import { ReportsPage } from './ReportsPage.tsx'

const fetchPeople = vi.fn<() => Promise<Person[]>>()
const fetchLatestReports = vi.fn<(personId: number) => Promise<LatestReportsResponse>>()
const fetchPersonReportChat = vi.fn<(personId: number) => Promise<PersonReportChatResponse>>()
const agentTurnPersonReportChat = vi.fn()

vi.mock('../api', async () => {
  const actual = await vi.importActual<typeof import('../api')>('../api')
  return {
    ...actual,
    fetchPeople: () => fetchPeople(),
    fetchLatestReports: (personId: number) => fetchLatestReports(personId),
    fetchPersonReportChat: (personId: number) => fetchPersonReportChat(personId),
    agentTurnPersonReportChat: (...args: unknown[]) => agentTurnPersonReportChat(...args),
    fetchPersonTrends: vi.fn(),
    markReportRead: vi.fn(),
    resolvePendingItem: vi.fn(),
  }
})

const person: Person = {
  id: 3,
  display_name: 'Alice',
  project_count: 1,
  unread_count: 0,
  open_pending_count: 0,
  identity_count: 1,
}

const emptyReports: LatestReportsResponse = {
  report_date: '2026-07-14',
  projects: [],
}

const emptyChat: PersonReportChatResponse = {
  agent_session_id: null,
  reviewer_agent: 'cursor',
  chat_messages: [],
}

function renderPage(personId = '3') {
  return render(
    <MemoryRouter initialEntries={[`/reports/${personId}`]}>
      <ToastProvider>
        <Routes>
          <Route path="/reports/:personId" element={<ReportsPage />} />
        </Routes>
      </ToastProvider>
    </MemoryRouter>,
  )
}

describe('ReportsPage Agent Chat', () => {
  beforeEach(() => {
    fetchPeople.mockReset()
    fetchLatestReports.mockReset()
    fetchPersonReportChat.mockReset()
    agentTurnPersonReportChat.mockReset()
    fetchPeople.mockResolvedValue([person])
    fetchLatestReports.mockResolvedValue(emptyReports)
    fetchPersonReportChat.mockResolvedValue(emptyChat)
  })

  it('rolls back optimistic user bubble and restores input on agent-turn failure', async () => {
    const user = userEvent.setup()
    agentTurnPersonReportChat.mockRejectedValue(new Error('agent failed'))
    renderPage()

    const input = await screen.findByPlaceholderText('例如：把 alpha 的 one_line 改得更精準')
    await user.type(input, 'tighten one_line')
    await user.click(screen.getByRole('button', { name: '送出' }))

    await waitFor(() => expect(agentTurnPersonReportChat).toHaveBeenCalledWith(3, 'tighten one_line'))
    // Bubble rolled back → empty hint returns; input restored (text lives in textarea only).
    await waitFor(() =>
      expect(screen.getByText('討論並調整這位人員的週報／觀察檔。')).toBeInTheDocument(),
    )
    expect(screen.getByPlaceholderText('例如：把 alpha 的 one_line 改得更精準')).toHaveValue(
      'tighten one_line',
    )
  })

  it('appends assistant reply and reloads latest reports on success', async () => {
    const user = userEvent.setup()
    agentTurnPersonReportChat.mockResolvedValue({
      reply: 'updated one_line for alpha',
      agent_session_id: 'session-1',
      ingest_warnings: [],
    })
    fetchLatestReports
      .mockResolvedValueOnce(emptyReports)
      .mockResolvedValueOnce({
        report_date: '2026-07-14',
        projects: [
          {
            id: 10,
            is_read: true,
            project_name: 'alpha',
            one_line: 'updated one_line for alpha',
            mr_count: 1,
            commit_count: 2,
            highlights: [],
            growth: [],
            pending_items: [],
            pending_observations: [],
          },
        ],
      })
    renderPage()

    const input = await screen.findByPlaceholderText('例如：把 alpha 的 one_line 改得更精準')
    await user.type(input, 'tighten one_line')
    await user.click(screen.getByRole('button', { name: '送出' }))

    expect(await screen.findByText('updated one_line for alpha')).toBeInTheDocument()
    expect(screen.getByText('tighten one_line')).toBeInTheDocument()
    await waitFor(() => expect(fetchLatestReports).toHaveBeenCalledTimes(2))
    expect(fetchLatestReports).toHaveBeenLastCalledWith(3)
  })
})
