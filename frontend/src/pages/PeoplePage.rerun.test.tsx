import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  bindIdentity,
  createPerson,
  fetchPeople,
  fetchPersonDetail,
  fetchUnmatchedAuthors,
  startPersonRun,
} from '../api'
import { ToastProvider } from '../context/ToastContext'
import { Toast } from '../components/layout/Toast.tsx'
import { PeoplePage } from './PeoplePage.tsx'

vi.mock('../api', () => ({
  bindIdentity: vi.fn(),
  createPerson: vi.fn(),
  fetchPeople: vi.fn(),
  fetchPersonDetail: vi.fn(),
  fetchUnmatchedAuthors: vi.fn(),
  renamePerson: vi.fn(),
  startPersonRun: vi.fn(),
  unbindIdentity: vi.fn(),
}))

const people = [
  {
    id: 1,
    display_name: 'Alice Chen',
    project_count: 1,
    unread_count: 0,
    open_pending_count: 0,
    identity_count: 1,
  },
]

function renderPage() {
  return render(
    <ToastProvider>
      <PeoplePage />
      <Toast />
    </ToastProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(fetchPeople).mockResolvedValue(people)
  vi.mocked(fetchUnmatchedAuthors).mockResolvedValue([])
  vi.mocked(fetchPersonDetail).mockResolvedValue({
    id: 1,
    display_name: 'Alice Chen',
    identities: [],
    projects: [{ id: 5, name: 'game-backend' }],
  })
})

afterEach(() => {
  cleanup()
})

describe('PeoplePage single-person rerun', () => {
  it('triggers manual_person run for the participated project', async () => {
    const user = userEvent.setup()
    vi.mocked(startPersonRun).mockResolvedValue({ run_id: 42 })
    renderPage()

    const rerun = await screen.findByText('重跑週報')
    await user.click(rerun)

    expect(startPersonRun).toHaveBeenCalledWith('game-backend', 1)
    expect(await screen.findByText(/已建立單人週報 run #42/)).toBeInTheDocument()
  })

  it('surfaces the backend error (e.g. 409) as a toast', async () => {
    const user = userEvent.setup()
    vi.mocked(startPersonRun).mockRejectedValue(new Error('a project is already queued or running'))
    renderPage()

    const rerun = await screen.findByText('重跑週報')
    await user.click(rerun)

    expect(startPersonRun).toHaveBeenCalledWith('game-backend', 1)
    expect(
      await screen.findByText(/a project is already queued or running/),
    ).toBeInTheDocument()
  })
})
