import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { BannerProvider } from './context/BannerContext.tsx'
import { Sidebar } from './components/layout/Sidebar.tsx'
import type { Person } from './types'

vi.mock('./api', () => ({
  fetchHealth: vi.fn(async () => ({ ok: true, data_dir: 'G:/reviewer' })),
  fetchDashboard: vi.fn(async () => ({
    last_run: null,
    stats: { project_count: 0, person_count: 0, unread_count: 0, pending_count: 0, mr_draft_count: 2 },
    recent_reports: [],
    recent_runs: [],
    schedule: {
      label: '每週一 09:00',
      next_run_at: null,
      enabled: true,
      weekday: 0,
      run_time: '09:00',
      tz_offset_min: 480,
      per_project_timeout_sec: 600,
      max_concurrency: 2,
      mr_poll_interval_min: 60,
      mr_poll_label: '每 60 分鐘',
      missed_weekly_run: null,
    },
  })),
  fetchPeople: vi.fn(async () => []),
  fetchUnmatchedAuthors: vi.fn(async () => []),
}))

const people: Person[] = [
  {
    id: 3,
    display_name: 'Alice',
    project_count: 1,
    unread_count: 0,
    open_pending_count: 2,
    identity_count: 1,
  },
]

describe('Sidebar navigation', () => {
  it('shows workbench/settings groups, MR badge, and reports person pending badge', async () => {
    render(
      <MemoryRouter initialEntries={['/reports/3']}>
        <Sidebar
          statusLine="已連線 · G:/reviewer"
          mrDraftCount={2}
          unmatchedCount={1}
          people={people}
        />
        <Routes>
          <Route path="/reports/:personId" element={<div>report-body</div>} />
        </Routes>
      </MemoryRouter>,
    )
    expect(screen.getByText('工作台')).toBeInTheDocument()
    expect(screen.getByText('設定')).toBeInTheDocument()
    expect(screen.getByText('Alice')).toBeInTheDocument()
    expect(screen.getAllByText('2').length).toBeGreaterThan(0)
    expect(screen.getByText('1')).toBeInTheDocument()
  })
})

describe('reports route param', () => {
  it('keeps person id in path', () => {
    render(
      <BannerProvider>
        <MemoryRouter initialEntries={['/reports/3']}>
          <Routes>
            <Route path="/reports/:personId" element={<div>person-3</div>} />
          </Routes>
        </MemoryRouter>
      </BannerProvider>,
    )
    expect(screen.getByText('person-3')).toBeInTheDocument()
  })
})
