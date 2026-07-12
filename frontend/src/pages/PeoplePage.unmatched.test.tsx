import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  bindIdentity,
  createPerson,
  fetchPeople,
  fetchPersonDetail,
  fetchUnmatchedAuthors,
} from '../api'
import { BannerProvider } from '../context/BannerContext'
import { PeoplePage } from './PeoplePage.tsx'

vi.mock('../api', () => ({
  bindIdentity: vi.fn(),
  createPerson: vi.fn(),
  fetchPeople: vi.fn(),
  fetchPersonDetail: vi.fn(),
  fetchUnmatchedAuthors: vi.fn(),
  renamePerson: vi.fn(),
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

const unmatched = [
  {
    id: 10,
    kind: 'git_email',
    value: 'dev@example.com',
    project_id: 2,
    project_name: 'game-backend',
    commit_count: 3,
    first_seen: '2026-07-12',
    last_seen: '2026-07-12',
  },
]

function renderPage() {
  return render(
    <BannerProvider>
      <PeoplePage />
    </BannerProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(fetchPeople).mockResolvedValue(people)
  vi.mocked(fetchUnmatchedAuthors).mockResolvedValue(unmatched)
  vi.mocked(fetchPersonDetail).mockImplementation(async (personId: number) => ({
    id: personId,
    display_name: personId === 1 ? 'Alice Chen' : 'Bob Wu',
    identities: [],
    projects: [],
  }))
  vi.mocked(bindIdentity).mockResolvedValue(undefined)
  vi.mocked(createPerson).mockResolvedValue({ id: 2, display_name: 'Bob Wu' })
})

afterEach(() => {
  cleanup()
})

describe('PeoplePage unmatched authors', () => {
  it('renders unmatched authors at the top of people page', async () => {
    renderPage()

    expect(await screen.findByRole('heading', { name: '未歸戶作者' })).toBeInTheDocument()
    expect(screen.getByText('dev@example.com')).toBeInTheDocument()
    expect(screen.getByText(/game-backend · 3 commits/)).toBeInTheDocument()
  })

  it('binds unmatched author to existing person', async () => {
    const user = userEvent.setup()
    renderPage()

    const row = (await screen.findByText('dev@example.com')).closest('article')
    expect(row).not.toBeNull()

    await user.selectOptions(
      within(row!).getByRole('combobox', { name: /綁定到現有人員/ }),
      '1',
    )
    await user.click(within(row!).getByRole('button', { name: '綁定' }))

    expect(bindIdentity).toHaveBeenCalledWith(1, 'git_email', 'dev@example.com')
  })

  it('creates a new person then binds unmatched author', async () => {
    const user = userEvent.setup()
    renderPage()

    const row = (await screen.findByText('dev@example.com')).closest('article')
    expect(row).not.toBeNull()

    await user.type(within(row!).getByRole('textbox', { name: /新顯示名稱/ }), 'Bob Wu')
    await user.click(within(row!).getByRole('button', { name: '建立並綁定' }))

    expect(createPerson).toHaveBeenCalledWith('Bob Wu')
    expect(bindIdentity).toHaveBeenCalledWith(2, 'git_email', 'dev@example.com')
  })
})
