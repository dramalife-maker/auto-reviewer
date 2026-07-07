import type {
  CreateRunResponse,
  HealthResponse,
  LatestReportsResponse,
  Person,
  RunStatus,
} from './types'
import { apiUrl } from './config'

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(apiUrl(path), init)
  if (!response.ok) {
    const text = await response.text()
    throw new Error(text || response.statusText)
  }
  if (response.status === 204) {
    return undefined as T
  }
  return (await response.json()) as T
}

export function fetchHealth(): Promise<HealthResponse> {
  return request('/health')
}

export function fetchPeople(): Promise<Person[]> {
  return request('/api/people')
}

export function fetchLatestReports(personId: number): Promise<LatestReportsResponse> {
  return request(`/api/people/${personId}/reports/latest`)
}

export function markReportRead(reportId: number): Promise<void> {
  return request(`/api/reports/${reportId}/read`, { method: 'PATCH' })
}

export function startManualRun(): Promise<CreateRunResponse> {
  return request('/api/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ trigger: 'manual_all' }),
  })
}

export function fetchRun(runId: number): Promise<RunStatus> {
  return request(`/api/runs/${runId}`)
}
