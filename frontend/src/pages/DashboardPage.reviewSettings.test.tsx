import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HashRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { Toast } from '../components/layout/Toast.tsx'
import { ToastProvider } from '../context/ToastContext.tsx'
import type { DashboardResponse } from '../types'
import { DashboardPage } from './DashboardPage.tsx'

vi.mock('../api', () => ({
  catchUpSchedule: vi.fn(),
  fetchDashboard: vi.fn(),
  fetchRun: vi.fn(),
  getReviewSettings: vi.fn(),
  startManualRun: vi.fn(),
  updateReviewSettings: vi.fn(),
  updateSchedule: vi.fn(),
}))

const api = await import('../api')

const dashboard: DashboardResponse = {
  last_run: null,
  stats: {
    project_count: 1,
    person_count: 1,
    unread_count: 0,
    pending_count: 0,
    mr_draft_count: 0,
  },
  recent_reports: [],
  recent_runs: [],
  schedule: {
    enabled: true,
    weekday: 0,
    run_time: '09:00',
    tz_offset_min: 480,
    per_project_timeout_sec: 600,
    max_concurrency: 2,
    mr_poll_interval_min: 60,
    label: '每週一 09:00',
    mr_poll_label: '每 60 分鐘',
    next_run_at: null,
    missed_weekly_run: null,
  },
}

function renderDashboard() {
  return render(
    <HashRouter>
      <ToastProvider>
        <DashboardPage />
        <Toast />
      </ToastProvider>
    </HashRouter>,
  )
}

describe('DashboardPage review ignore list', () => {
  beforeEach(() => {
    vi.mocked(api.fetchDashboard).mockResolvedValue(dashboard)
    vi.mocked(api.getReviewSettings).mockResolvedValue({ ignore_globs: ['*.lock'] })
  })

  afterEach(() => {
    cleanup()
    vi.clearAllMocks()
  })

  it('loads the stored list into the editor', async () => {
    renderDashboard()
    await waitFor(() => {
      expect(screen.getByLabelText('忽略清單')).toHaveValue('*.lock')
    })
  })

  it('sends lines verbatim and reflects the normalized response', async () => {
    vi.mocked(api.updateReviewSettings).mockResolvedValue({
      ignore_globs: ['*.lock', 'vendor/**'],
    })
    renderDashboard()
    const editor = await screen.findByLabelText('忽略清單')

    await userEvent.clear(editor)
    // Blank line and stray whitespace stay in: the backend normalizes, not us.
    await userEvent.type(editor, '  *.lock  \n\nvendor/**')
    await userEvent.click(screen.getByRole('button', { name: '儲存忽略清單' }))

    await waitFor(() => {
      expect(api.updateReviewSettings).toHaveBeenCalledWith({
        ignore_globs: ['  *.lock  ', '', 'vendor/**'],
      })
    })
    await waitFor(() => {
      expect(screen.getByLabelText('忽略清單')).toHaveValue('*.lock\nvendor/**')
    })
    expect(await screen.findByText(/^忽略清單已儲存/)).toBeInTheDocument()
  })

  it('surfaces a rejected save without claiming success', async () => {
    vi.mocked(api.updateReviewSettings).mockRejectedValue(
      new Error("ignore glob must not start with ':'"),
    )
    renderDashboard()
    const editor = await screen.findByLabelText('忽略清單')

    await userEvent.clear(editor)
    await userEvent.type(editor, ':(top)')
    await userEvent.click(screen.getByRole('button', { name: '儲存忽略清單' }))

    expect(await screen.findByText(/must not start with/)).toBeInTheDocument()
    expect(screen.queryByText(/^忽略清單已儲存/)).not.toBeInTheDocument()
  })
})
