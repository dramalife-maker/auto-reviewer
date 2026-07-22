import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ToastProvider } from '../context/ToastContext.tsx'
import type { RunListItem, RunStatus } from '../types'
import { RunsPage } from './RunsPage.tsx'

const fetchRuns = vi.fn<() => Promise<{ runs: RunListItem[]; total: number }>>()
const fetchRun = vi.fn<(id: number) => Promise<RunStatus>>()
const cancelRun = vi.fn<(id: number) => Promise<RunStatus>>()

vi.mock('../api', async () => {
  const actual = await vi.importActual<typeof import('../api')>('../api')
  return {
    ...actual,
    fetchRuns: () => fetchRuns(),
    fetchRun: (id: number) => fetchRun(id),
    cancelRun: (id: number) => cancelRun(id),
  }
})

function baseRun(overrides: Partial<RunStatus> = {}): RunStatus {
  return {
    id: 7,
    trigger: 'mr_poll',
    status: 'success',
    started_at: '2026-07-05 09:00:00',
    finished_at: '2026-07-05 09:05:00',
    duration_sec: 300,
    note: null,
    project_total: 1,
    project_skipped: 0,
    projects: [
      {
        name: 'game-backend',
        state: 'done',
        error: null,
        started_at: '2026-07-05 09:00:00',
        finished_at: '2026-07-05 09:05:00',
        duration_sec: 300,
      },
    ],
    ...overrides,
  }
}

function renderPage(run: RunStatus) {
  fetchRuns.mockResolvedValue({
    runs: [
      {
        id: run.id,
        trigger: run.trigger,
        status: run.status,
        started_at: run.started_at,
        finished_at: run.finished_at,
        duration_sec: run.duration_sec,
        project_total: run.project_total,
        project_skipped: run.project_skipped,
      },
    ],
    total: 1,
  })
  fetchRun.mockResolvedValue(run)

  return render(
    <MemoryRouter initialEntries={[`/runs/${run.id}`]}>
      <ToastProvider>
        <Routes>
          <Route path="/runs/:runId" element={<RunsPage />} />
          <Route path="/mr-inbox" element={<div>inbox</div>} />
          <Route path="/reports/:personId" element={<div>report</div>} />
        </Routes>
      </ToastProvider>
    </MemoryRouter>,
  )
}

const CANCEL_LABEL = '中止執行'

describe('RunsPage outputs hints', () => {
  beforeEach(() => {
    fetchRuns.mockReset()
    fetchRun.mockReset()
    cancelRun.mockReset()
  })

  it('shows MR draft count with link to inbox', async () => {
    renderPage(
      baseRun({
        projects: [
          {
            name: 'game-backend',
            state: 'done',
            error: null,
            started_at: null,
            finished_at: null,
            duration_sec: 10,
            outputs: { mr_drafts: { count: 2 }, weekly_reports: null },
          },
        ],
      }),
    )

    expect(await screen.findByText(/已產出 2 份 MR 草稿/)).toBeInTheDocument()
    const link = screen.getByRole('link', { name: 'MR 收件匣' })
    expect(link).toHaveAttribute('href', '/mr-inbox')
  })

  it('shows weekly people as links to reports', async () => {
    renderPage(
      baseRun({
        trigger: 'schedule',
        projects: [
          {
            name: 'game-backend',
            state: 'done',
            error: null,
            started_at: null,
            finished_at: null,
            duration_sec: 10,
            outputs: {
              mr_drafts: null,
              weekly_reports: {
                people: [
                  { person_id: 1, display_name: 'Alice' },
                  { person_id: 2, display_name: 'Bob' },
                ],
              },
            },
          },
        ],
      }),
    )

    expect(await screen.findByText(/的週報/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Alice' })).toHaveAttribute('href', '/reports/1')
    expect(screen.getByRole('link', { name: 'Bob' })).toHaveAttribute('href', '/reports/2')
  })

  it('hides outputs hints when outputs is absent', async () => {
    renderPage(baseRun())

    await waitFor(() => {
      expect(screen.getByText('game-backend')).toBeInTheDocument()
    })
    expect(screen.queryByText(/已產出/)).not.toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'MR 收件匣' })).not.toBeInTheDocument()
  })
})

describe('RunsPage cancelled status (task 5.1)', () => {
  beforeEach(() => {
    fetchRuns.mockReset()
    fetchRun.mockReset()
    cancelRun.mockReset()
  })

  it('renders a cancelled run and project distinctly from failed', async () => {
    renderPage(
      baseRun({
        status: 'cancelled',
        projects: [
          {
            name: 'game-backend',
            state: 'cancelled',
            error: null,
            started_at: null,
            finished_at: null,
            duration_sec: 3,
          },
        ],
      }),
    )

    // Both the run header and the project row surface the cancelled status.
    const pills = await screen.findAllByText('cancelled')
    expect(pills.length).toBeGreaterThanOrEqual(2)
    // Distinct from failed: cancelled uses the neutral tone, never the danger tone.
    for (const pill of pills) {
      expect(pill.className).toContain('text-ink-muted')
      expect(pill.className).not.toContain('text-danger')
    }
  })
})

describe('RunsPage cancel action (task 5.2)', () => {
  beforeEach(() => {
    fetchRuns.mockReset()
    fetchRun.mockReset()
    cancelRun.mockReset()
  })

  it('offers a cancel action on a running run', async () => {
    renderPage(baseRun({ status: 'running', finished_at: null }))
    expect(await screen.findByRole('button', { name: CANCEL_LABEL })).toBeInTheDocument()
  })

  it('does not offer a cancel action on a terminal run', async () => {
    renderPage(baseRun({ status: 'success' }))
    await waitFor(() => {
      expect(screen.getByText('game-backend')).toBeInTheDocument()
    })
    expect(screen.queryByRole('button', { name: CANCEL_LABEL })).not.toBeInTheDocument()
  })

  it('reflects cancelled status after triggering cancellation without a reload', async () => {
    const running = baseRun({ status: 'running', finished_at: null })
    renderPage(running)

    // Wait for the initial (running) detail before flipping to cancelled.
    const button = await screen.findByRole('button', { name: CANCEL_LABEL })

    const cancelled = baseRun({
      status: 'cancelled',
      finished_at: '2026-07-05 09:02:00',
      projects: [
        {
          name: 'game-backend',
          state: 'cancelled',
          error: null,
          started_at: null,
          finished_at: null,
          duration_sec: 3,
        },
      ],
    })
    cancelRun.mockResolvedValue(cancelled)
    // Once cancelled server-side, subsequent reads also return cancelled.
    fetchRun.mockResolvedValue(cancelled)

    fireEvent.click(button)

    await waitFor(() => {
      expect(cancelRun).toHaveBeenCalledWith(7)
    })
    // Status updates in place (no manual reload); the cancel action is gone.
    await screen.findAllByText('cancelled')
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: CANCEL_LABEL })).not.toBeInTheDocument()
    })
  })
})
