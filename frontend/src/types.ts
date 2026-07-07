export interface HealthResponse {
  status: string
  data_dir: string
}

export interface Person {
  id: number
  display_name: string
  project_count: number
  unread_count: number
  open_pending_count: number
  identity_count: number
}

export interface UnmatchedAuthor {
  id: number
  kind: string
  value: string
  project_id: number | null
  project_name: string | null
  commit_count: number
  first_seen: string
  last_seen: string
}

export interface CreatePersonResponse {
  id: number
  display_name: string
}

export interface LatestReportItem {
  id: number
  is_read: boolean
  project_name: string
  one_line: string | null
  mr_count: number | null
  commit_count: number | null
  highlights: string[]
  growth: string[]
  pending: string[]
}

export interface LatestReportsResponse {
  report_date: string
  projects: LatestReportItem[]
}

export interface RunStatus {
  id: number
  trigger: string
  status: string
  started_at: string
  finished_at: string | null
  project_total: number | null
  project_skipped: number
}

export interface CreateRunResponse {
  run_id: number
}

export interface ProjectHealth {
  name: string
  health: string
  health_reason: string | null
  is_git_repo: number
}

export interface ReloadProjectsResponse {
  total: number
  healthy: number
  unhealthy: number
  projects: ProjectHealth[]
}
