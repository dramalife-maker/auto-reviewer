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
