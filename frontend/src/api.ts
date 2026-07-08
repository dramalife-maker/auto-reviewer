import type {
  CreatePersonResponse,
  CreateRunResponse,
  DashboardResponse,
  HealthResponse,
  LatestReportsResponse,
  Person,
  PersonTrendsResponse,
  ProjectListResponse,
  ProjectInput,
  ProjectListItem,
  ProjectUpdateInput,
  ReloadProjectsResponse,
  RunStatus,
  UnmatchedAuthor,
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

export function fetchDashboard(): Promise<DashboardResponse> {
  return request('/api/dashboard')
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

export function fetchPersonTrends(personId: number): Promise<PersonTrendsResponse> {
  return request(`/api/people/${personId}/trends`)
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

export function reloadProjects(): Promise<ReloadProjectsResponse> {
  return request('/api/projects/reload', { method: 'POST' })
}

export function fetchProjects(): Promise<ProjectListResponse> {
  return request('/api/projects')
}

export function createProject(body: ProjectInput): Promise<ProjectListItem> {
  return request('/api/projects', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export function updateProject(name: string, body: ProjectUpdateInput): Promise<ProjectListItem> {
  return request(`/api/projects/${encodeURIComponent(name)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export function deleteProject(name: string): Promise<void> {
  return request(`/api/projects/${encodeURIComponent(name)}`, { method: 'DELETE' })
}

export function fetchUnmatchedAuthors(): Promise<UnmatchedAuthor[]> {
  return request('/api/unmatched-authors')
}

export function createPerson(displayName: string): Promise<CreatePersonResponse> {
  return request('/api/people', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ display_name: displayName }),
  })
}

export function bindIdentity(
  personId: number,
  kind: string,
  value: string,
  label?: string,
): Promise<void> {
  return request(`/api/people/${personId}/identities`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ kind, value, label }),
  })
}
