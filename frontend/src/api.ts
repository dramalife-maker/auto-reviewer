import type {
  CreatePersonResponse,
  CreateRunResponse,
  DashboardResponse,
  HealthResponse,
  LatestReportsResponse,
  MrReviewAgentTurnResponse,
  MrReviewItem,
  MrReviewPublishResponse,
  MrReviewStatus,
  PendingItem,
  Person,
  PersonDetail,
  PersonTrendsResponse,
  ProjectListResponse,
  ProjectInput,
  ProjectListItem,
  ProjectUpdateInput,
  ReloadProjectsResponse,
  ReviewSettings,
  RunStatus,
  RunsListResponse,
  ScheduleConfigResponse,
  ScheduleUpdateInput,
  UnmatchedAuthor,
  PersonReportChatAgentTurnResponse,
  PersonReportChatResponse,
} from './types'
import { apiUrl } from './config'

export class ApiError extends Error {
  readonly status: number
  readonly body: string

  constructor(message: string, status: number, body = '') {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.body = body
  }

  parseJson(): unknown | null {
    if (!this.body) return null
    try {
      return JSON.parse(this.body) as unknown
    } catch {
      return null
    }
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(apiUrl(path), init)
  if (!response.ok) {
    const text = await response.text()
    throw new ApiError(text || response.statusText, response.status, text)
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

export function resolvePendingItem(itemId: number, resolutionNote?: string): Promise<PendingItem> {
  return request(`/api/pending-items/${itemId}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ status: 'resolved', resolution_note: resolutionNote }),
  })
}

export function startManualRun(): Promise<CreateRunResponse> {
  return request('/api/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ trigger: 'manual_all' }),
  })
}

export function startProjectRun(projectName: string): Promise<CreateRunResponse> {
  return request('/api/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ trigger: 'manual_project', project_name: projectName }),
  })
}

export function startPersonRun(
  projectName: string,
  personId: number,
): Promise<CreateRunResponse> {
  return request('/api/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      trigger: 'manual_person',
      project_name: projectName,
      person_id: personId,
    }),
  })
}

export function fetchRun(runId: number): Promise<RunStatus> {
  return request(`/api/runs/${runId}`)
}

export function cancelRun(runId: number): Promise<RunStatus> {
  return request(`/api/runs/${runId}/cancel`, { method: 'POST' })
}

export function fetchRuns(params?: {
  limit?: number
  offset?: number
  trigger?: string
  status?: string
}): Promise<RunsListResponse> {
  const search = new URLSearchParams()
  if (params?.limit != null) search.set('limit', String(params.limit))
  if (params?.offset != null) search.set('offset', String(params.offset))
  if (params?.trigger) search.set('trigger', params.trigger)
  if (params?.status) search.set('status', params.status)
  const query = search.toString()
  return request(`/api/runs${query ? `?${query}` : ''}`)
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

export function fetchPersonDetail(personId: number): Promise<PersonDetail> {
  return request(`/api/people/${personId}`)
}

export function renamePerson(personId: number, displayName: string): Promise<PersonDetail> {
  return request(`/api/people/${personId}`, {
    method: 'PATCH',
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

export function unbindIdentity(personId: number, identityId: number): Promise<void> {
  return request(`/api/people/${personId}/identities/${identityId}`, {
    method: 'DELETE',
  })
}

export function fetchMrReviews(status?: MrReviewStatus): Promise<MrReviewItem[]> {
  const query = status ? `?status=${encodeURIComponent(status)}` : ''
  return request(`/api/mr-reviews${query}`)
}

export function updateMrReview(
  id: number,
  draftBody: string,
  baseHash?: string,
): Promise<void> {
  return request(`/api/mr-reviews/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      draft_body: draftBody,
      ...(baseHash !== undefined ? { base_hash: baseHash } : {}),
    }),
  })
}

export function publishMrReview(id: number): Promise<MrReviewPublishResponse> {
  return request(`/api/mr-reviews/${id}/publish`, { method: 'POST' })
}

export function ignoreMrReview(id: number): Promise<void> {
  return request(`/api/mr-reviews/${id}/ignore`, { method: 'POST' })
}

export function restoreMrReview(id: number): Promise<void> {
  return request(`/api/mr-reviews/${id}/restore`, { method: 'POST' })
}

export function agentTurnMrReview(
  id: number,
  message: string,
): Promise<MrReviewAgentTurnResponse> {
  return request(`/api/mr-reviews/${id}/agent-turn`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message }),
  })
}

export function fetchPersonReportChat(personId: number): Promise<PersonReportChatResponse> {
  return request(`/api/people/${personId}/report-chat`)
}

export function agentTurnPersonReportChat(
  personId: number,
  message: string,
): Promise<PersonReportChatAgentTurnResponse> {
  return request(`/api/people/${personId}/report-chat/agent-turn`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message }),
  })
}

export function startMrScan(projectId: number, options?: { force?: boolean }): Promise<CreateRunResponse> {
  const query = options?.force ? '?force=1' : ''
  return request(`/api/projects/${projectId}/mr-scan${query}`, { method: 'POST' })
}

export function updateSchedule(body: ScheduleUpdateInput): Promise<ScheduleConfigResponse> {
  return request('/api/schedule', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export function getReviewSettings(): Promise<ReviewSettings> {
  return request('/api/review-settings')
}

export function updateReviewSettings(body: ReviewSettings): Promise<ReviewSettings> {
  return request('/api/review-settings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export function catchUpSchedule(): Promise<CreateRunResponse> {
  return request('/api/schedule/catch-up', { method: 'POST' })
}
