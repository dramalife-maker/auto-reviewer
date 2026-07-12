import { useCallback, useEffect, useState } from 'react'

import {
  fetchDashboard,
  fetchHealth,
  fetchMrReviews,
  fetchPeople,
  fetchProjects,
  fetchRuns,
  fetchUnmatchedAuthors,
  updateSchedule,
} from '../api'
import type {
  DashboardResponse,
  HealthResponse,
  MrReviewItem,
  MrReviewStatus,
  Person,
  ProjectListResponse,
  RunsListResponse,
  ScheduleUpdateInput,
  UnmatchedAuthor,
} from '../types'

export function useHealth() {
  const [health, setHealth] = useState<HealthResponse | null>(null)
  const [error, setError] = useState<Error | null>(null)
  const reload = useCallback(async () => {
    try {
      const next = await fetchHealth()
      setHealth(next)
      setError(null)
      return next
    } catch (err) {
      const error = err instanceof Error ? err : new Error('health failed')
      setError(error)
      throw error
    }
  }, [])
  return { health, error, reload }
}

export function useDashboard() {
  const [dashboard, setDashboard] = useState<DashboardResponse | null>(null)
  const reload = useCallback(async () => {
    const next = await fetchDashboard()
    setDashboard(next)
    return next
  }, [])
  return { dashboard, setDashboard, reload }
}

export function usePeople() {
  const [people, setPeople] = useState<Person[]>([])
  const reload = useCallback(async () => {
    const next = await fetchPeople()
    setPeople(next)
    return next
  }, [])
  return { people, setPeople, reload }
}

export function useUnmatchedAuthors() {
  const [authors, setAuthors] = useState<UnmatchedAuthor[]>([])
  const reload = useCallback(async () => {
    try {
      const next = await fetchUnmatchedAuthors()
      setAuthors(next)
      return next
    } catch {
      setAuthors([])
      return [] as UnmatchedAuthor[]
    }
  }, [])
  return { authors, setAuthors, reload }
}

export function useProjects() {
  const [projects, setProjects] = useState<ProjectListResponse | null>(null)
  const reload = useCallback(async () => {
    const next = await fetchProjects()
    setProjects(next)
    return next
  }, [])
  return { projects, reload }
}

export function useMrReviews(status?: MrReviewStatus) {
  const [items, setItems] = useState<MrReviewItem[]>([])
  const reload = useCallback(async () => {
    const next = await fetchMrReviews(status)
    setItems(next)
    return next
  }, [status])
  useEffect(() => {
    void reload().catch(() => setItems([]))
  }, [reload])
  return { items, reload }
}

export function useRuns(params?: { limit?: number; offset?: number }) {
  const [data, setData] = useState<RunsListResponse | null>(null)
  const reload = useCallback(async () => {
    const next = await fetchRuns(params)
    setData(next)
    return next
  }, [params?.limit, params?.offset])
  return { data, reload }
}

export function useScheduleSave() {
  return useCallback((body: ScheduleUpdateInput) => updateSchedule(body), [])
}
