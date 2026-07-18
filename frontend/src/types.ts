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

export interface IdentityItem {
  id: number
  kind: string
  value: string
  label: string | null
}

export interface PersonProjectItem {
  id: number
  name: string
}

export interface PersonDetail {
  id: number
  display_name: string
  identities: IdentityItem[]
  projects: PersonProjectItem[]
}

export type IdentityKind = 'git_email' | 'gitlab_user' | 'glab_user'

export interface PendingItem {
  id: number
  person_id: number
  project_id: number
  project_name: string
  report_id: number | null
  question: string
  status: 'open' | 'resolved'
  raised_date: string
  resolved_date: string | null
  resolution_note: string | null
}

export type PendingObservationStatus = 'draft' | 'published' | 'ignored' | 'unknown'

export interface PendingObservation {
  mr_iid: number
  review_round: number
  mr_title: string | null
  status: PendingObservationStatus
  filename: string
  content: string
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
  pending_items: PendingItem[]
  pending_observations: PendingObservation[]
}

export interface LatestReportsResponse {
  report_date: string | null
  projects: LatestReportItem[]
}

export interface GrowthTimelineEntry {
  month: string
  content: string
}

export interface HistoricalPendingEntry {
  question: string
  status: 'open' | 'resolved'
  raised_month: string
  resolved_month: string | null
  resolution_note: string | null
  raw_line: string
}

export interface PersonTrendsResponse {
  person_id: number
  display_name: string
  long_term_observation: string
  growth_timeline: GrowthTimelineEntry[]
  historical_pending: HistoricalPendingEntry[]
}

export interface RunProjectStatus {
  name: string
  state: string
  error: string | null
  started_at: string | null
  finished_at: string | null
  duration_sec: number | null
  skip_summary?: SkipSummary | null
  outputs?: ProjectOutputs | null
}

export interface SkipSummaryItem {
  mr_iid: number
  skip_reason: string
}

export interface SkipSummary {
  by_reason: Record<string, number>
  items: SkipSummaryItem[]
}

export interface MrDraftsOutput {
  count: number
}

export interface WeeklyReportPerson {
  person_id: number
  display_name: string
}

export interface WeeklyReportsOutput {
  people: WeeklyReportPerson[]
}

export interface ProjectOutputs {
  mr_drafts?: MrDraftsOutput | null
  weekly_reports?: WeeklyReportsOutput | null
}

export interface RunListItem {
  id: number
  trigger: string
  status: string
  started_at: string
  finished_at: string | null
  duration_sec: number | null
  project_total: number | null
  project_skipped: number
}

export interface RunsListResponse {
  runs: RunListItem[]
  total: number
}

export interface RunStatus {
  id: number
  trigger: string
  status: string
  started_at: string
  finished_at: string | null
  duration_sec: number | null
  note: string | null
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

export interface ProjectEngineer {
  display_name: string
  gitlab_username: string | null
}

export interface ProjectListItem {
  id: number
  name: string
  repo_path: string
  git_remote_url: string | null
  default_branch: string | null
  default_branches: string[]
  mr_review_skip_labels: string[]
  mr_review_require_label: string | null
  health: string
  health_reason: string | null
  is_git_repo: number
  source_type: 'gitlab' | 'local'
  last_report_date: string | null
  engineers: ProjectEngineer[]
}

export interface ProjectListResponse {
  projects: ProjectListItem[]
}

export interface ProjectInput {
  name: string
  source_type: 'gitlab' | 'local'
  repo_path: string
  git_remote_url?: string | null
  default_branches?: string[]
  mr_review_skip_labels?: string[]
  mr_review_require_label?: string | null
}

export interface ProjectUpdateInput {
  source_type: 'gitlab' | 'local'
  repo_path: string
  git_remote_url?: string | null
  default_branches?: string[]
  mr_review_skip_labels?: string[]
  mr_review_require_label?: string | null
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
  mr_draft_count: number
}

export interface DashboardRecentReport {
  report_id: number
  person_id: number
  person_name: string
  project_name: string
  is_read: boolean
  pending_count: number
}

export interface MissedWeeklyRun {
  due_at: string
  label: string
}

export interface DashboardSchedule {
  label: string
  next_run_at: string | null
  enabled: boolean
  weekday: number | null
  run_time: string
  tz_offset_min: number
  per_project_timeout_sec: number
  max_concurrency: number
  mr_poll_interval_min: number
  mr_poll_label: string
  missed_weekly_run: MissedWeeklyRun | null
}

export interface ScheduleConfigResponse {
  enabled: boolean
  cadence: string
  weekday: number | null
  run_time: string
  tz_offset_min: number
  mr_poll_interval_min: number
  per_project_timeout_sec: number
  max_concurrency: number
  weekly_label: string
  mr_poll_label: string
  next_weekly_run_at: string | null
  missed_weekly_run: MissedWeeklyRun | null
}

export interface ScheduleUpdateInput {
  enabled?: boolean
  weekday?: number
  run_time?: string
  tz_offset_min?: number
  per_project_timeout_sec?: number
  max_concurrency?: number
  mr_poll_interval_min?: number
  cadence?: string
}

export interface DashboardResponse {
  last_run: DashboardLastRun | null
  stats: DashboardStats
  recent_reports: DashboardRecentReport[]
  recent_runs: RunListItem[]
  schedule: DashboardSchedule
}

export type MrReviewStatus = 'draft' | 'published' | 'ignored'

export interface MrReviewChatMessage {
  id: number
  role: 'user' | 'assistant'
  content: string
  created_at: string
}

export interface MrReviewItem {
  id: number
  project_id: number
  project_name: string
  person_id: number | null
  author_name: string | null
  mr_iid: number
  mr_title: string | null
  review_round: number
  status: MrReviewStatus
  draft_body: string
  draft_hash: string
  chat_messages: MrReviewChatMessage[]
  agent_session_id: string | null
  reviewer_agent: string
  created_at: string
}

export interface MrReviewPublishResponse {
  published_at: string
  published_body: string
}

export interface MrReviewAgentTurnResponse {
  reply: string
  agent_session_id: string
  draft_body: string
  draft_hash: string
}

export interface PersonReportChatMessage {
  id: number
  role: 'user' | 'assistant'
  content: string
  created_at: string
}

export interface PersonReportChatResponse {
  agent_session_id: string | null
  reviewer_agent: string
  chat_messages: PersonReportChatMessage[]
}

export interface PersonReportChatAgentTurnResponse {
  reply: string
  agent_session_id: string | null
  ingest_warnings?: string[]
}

export interface MrReviewDraftConflict {
  draft_body: string
  draft_hash: string
}
