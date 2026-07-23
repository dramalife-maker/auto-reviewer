import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HashRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

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

function dashboardWithMissed(dueAt: string): DashboardResponse {
  return {
    last_run: null,
    stats: {
      project_count: 1,
      person_count: 2,
      unread_count: 3,
      pending_count: 4,
      mr_draft_count: 5,
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
      missed_weekly_run: {
        due_at: dueAt,
        label: '2026-07-06 09:00',
      },
    },
  }
}

function renderDashboard() {
  return render(
    <HashRouter>
      <ToastProvider>
        <DashboardPage />
      </ToastProvider>
    </HashRouter>,
  )
}

describe('DashboardPage catch-up banner', () => {
  beforeEach(() => {
    sessionStorage.clear()
    vi.mocked(api.fetchDashboard).mockResolvedValue(dashboardWithMissed('2026-07-06T01:00:00Z'))
    vi.mocked(api.getReviewSettings).mockResolvedValue({ ignore_globs: [] })
  })

  afterEach(() => {
    cleanup()
    vi.clearAllMocks()
    sessionStorage.clear()
  })

  it('shows catch-up banner when a weekly run was missed', async () => {
    renderDashboard()

    expect(await screen.findByText(/錯過週報排程/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '立即補跑' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '稍後' })).toBeInTheDocument()
  })

  it('hides catch-up banner after dismissing it in this tab', async () => {
    const user = userEvent.setup()
    renderDashboard()

    await user.click(await screen.findByRole('button', { name: '稍後' }))

    expect(screen.queryByText(/錯過週報排程/)).not.toBeInTheDocument()
  })

  it('shows catch-up banner again when the missed due_at changes', async () => {
    const user = userEvent.setup()
    renderDashboard()

    await user.click(await screen.findByRole('button', { name: '稍後' }))
    await waitFor(() => expect(screen.queryByText(/錯過週報排程/)).not.toBeInTheDocument())

    cleanup()
    vi.mocked(api.fetchDashboard).mockResolvedValue(dashboardWithMissed('2026-07-13T01:00:00Z'))
    renderDashboard()

    expect(await screen.findByText(/錯過週報排程/)).toBeInTheDocument()
  })
})
