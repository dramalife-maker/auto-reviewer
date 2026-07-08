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

export interface GrowthTimelineEntry {
  month: string
  content: string
}

export interface PersonTrendsResponse {
  person_id: number
  display_name: string
  long_term_observation: string
  growth_timeline: GrowthTimelineEntry[]
  historical_pending: string[]
}

export interface RunProjectStatus {
  name: string
  state: string
  error: string | null
}

export interface RunStatus {
  id: number
  trigger: string
  status: string
  started_at: string
  finished_at: string | null
  project_total: number | null
  project_skipped: number
  projects: RunProjectStatus[]
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

export interface DashboardLastRun {
  started_at: string
  duration_sec: number | null
  status: string
}

export interface DashboardStats {
  project_count: number
  person_count: number
  unread_count: number
  pending_count: number
}

export interface DashboardRecentReport {
  report_id: number
  person_id: number
  person_name: string
  project_name: string
  is_read: boolean
  pending_count: number
}

export interface DashboardSchedule {
  label: string
  next_run_at: string | null
  enabled: boolean
}

export interface DashboardResponse {
  last_run: DashboardLastRun | null
  stats: DashboardStats
  recent_reports: DashboardRecentReport[]
  schedule: DashboardSchedule
}
