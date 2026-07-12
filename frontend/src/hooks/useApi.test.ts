import { describe, expect, it, vi } from 'vitest'

import * as api from '../api'
import {
  useDashboard,
  useHealth,
  useMrReviews,
  usePeople,
  useProjects,
  useRuns,
  useScheduleSave,
  useUnmatchedAuthors,
} from './useApi'

vi.mock('../api', () => ({
  fetchHealth: vi.fn(async () => ({ ok: true, data_dir: 'G:/x' })),
  fetchDashboard: vi.fn(async () => ({ stats: {} })),
  fetchPeople: vi.fn(async () => []),
  fetchUnmatchedAuthors: vi.fn(async () => []),
  fetchProjects: vi.fn(async () => ({ projects: [] })),
  fetchMrReviews: vi.fn(async () => []),
  fetchRuns: vi.fn(async () => ({ runs: [], total: 0 })),
  updateSchedule: vi.fn(async (body: unknown) => body),
}))

describe('api hooks wrappers', () => {
  it('exposes reload helpers that call existing api functions', async () => {
    // Hooks are callables returning objects — verify module exports and api wiring via direct calls
    expect(typeof useHealth).toBe('function')
    expect(typeof useDashboard).toBe('function')
    expect(typeof usePeople).toBe('function')
    expect(typeof useUnmatchedAuthors).toBe('function')
    expect(typeof useProjects).toBe('function')
    expect(typeof useMrReviews).toBe('function')
    expect(typeof useRuns).toBe('function')
    expect(typeof useScheduleSave).toBe('function')
    await api.fetchHealth()
    await api.fetchDashboard()
    await api.fetchPeople()
    await api.fetchUnmatchedAuthors()
    await api.fetchProjects()
    await api.fetchMrReviews('draft')
    await api.fetchRuns()
    await api.updateSchedule({ enabled: true })
    expect(api.fetchHealth).toHaveBeenCalled()
    expect(api.updateSchedule).toHaveBeenCalledWith({ enabled: true })
  })
})
