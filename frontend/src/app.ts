import {
  agentTurnMrReview,
  bindIdentity,
  createPerson,
  createProject,
  deleteProject,
  fetchDashboard,
  fetchHealth,
  fetchLatestReports,
  fetchMrReviews,
  fetchPeople,
  fetchPersonDetail,
  fetchPersonTrends,
  fetchProjects,
  fetchRun,
  fetchUnmatchedAuthors,
  ignoreMrReview,
  markReportRead,
  publishMrReview,
  reloadProjects,
  renamePerson,
  resolvePendingItem,
  startManualRun,
  startMrScan,
  startProjectRun,
  unbindIdentity,
  updateMrReview,
  updateProject,
  updateSchedule,
  ApiError,
} from './api'
import type {
  DashboardResponse,
  IdentityKind,
  LatestReportItem,
  LatestReportsResponse,
  MrReviewItem,
  MrReviewStatus,
  PendingItem,
  Person,
  PersonDetail,
  PersonTrendsResponse,
  ProjectListItem,
  RunStatus,
  UnmatchedAuthor,
} from './types'

const TERMINAL_STATUSES = new Set(['success', 'partial', 'failed'])
const DEFAULT_MR_SKIP_LABELS = ['wip', 'do-not-review', 'no-ai-review']
const IDENTITY_KINDS: Array<{ value: IdentityKind; label: string }> = [
  { value: 'git_email', label: 'Git email' },
  { value: 'gitlab_user', label: 'GitLab username' },
  { value: 'glab_user', label: 'glab user' },
]
type AppView = 'dashboard' | 'reports' | 'projects' | 'people' | 'mr-inbox'
type SourceType = 'gitlab' | 'local'

interface ProjectDraft {
  name: string
  source_type: SourceType
  repo_path: string
  git_remote_url: string
  default_branches: string
  mr_review_skip_labels: string
  mr_review_require_label: string
  isNew: boolean
}

export class ReviewerApp {
  private root: HTMLElement
  private people: Person[] = []
  private selectedPersonId: number | null = null
  private personViewTab: 'weekly' | 'trends' = 'weekly'
  private latestReports: LatestReportsResponse | null = null
  private personTrends: PersonTrendsResponse | null = null
  private activeRunId: number | null = null
  private activeRun: RunStatus | null = null
  private runElapsedTimer: number | null = null
  private runElapsedTick = 0
  private pollTimer: number | null = null
  private bannerMessage: string | null = null
  private bannerIsError = false
  private reloading = false
  private unmatchedAuthors: UnmatchedAuthor[] = []
  private showUnmatchedPanel = false
  private appView: AppView = 'dashboard'
  private dashboard: DashboardResponse | null = null
  private projects: ProjectListItem[] = []
  private selectedProjectName: string | null = null
  private projectDraft: ProjectDraft | null = null
  private isCreatingProject = false
  private selectedSettingsPersonId: number | null = null
  private personDetail: PersonDetail | null = null
  private isCreatingPerson = false
  private newPersonName = ''
  private identityDraftKind: IdentityKind = 'git_email'
  private identityDraftValue = ''
  private peopleSettingsSaving = false
  private mrReviews: MrReviewItem[] = []
  private selectedMrReviewId: number | null = null
  private mrStatusFilter: MrReviewStatus = 'draft'
  private mrEditorBody = ''
  private mrEditorDirty = false
  private mrActionLoading = false
  private mrChatMessages: Array<{ role: 'user' | 'assistant'; text: string }> = []
  private mrChatLoading = false
  private scheduleSaving = false
  private resolvingPendingItemIds = new Set<number>()

  constructor(root: HTMLElement) {
    this.root = root
  }

  async init(): Promise<void> {
    try {
      const health = await fetchHealth()
      this.bannerMessage = null
      await Promise.all([
        this.loadPeople(),
        this.loadUnmatchedAuthors(),
        this.loadDashboard(),
        this.loadProjects(),
      ])
      this.render(`已連線 · ${health.data_dir}`)
    } catch (error) {
      this.renderError(error)
    }
  }

  private async loadPeople(): Promise<void> {
    this.people = await fetchPeople()
    if (this.selectedPersonId === null && this.people.length > 0) {
      this.selectedPersonId = this.people[0].id
    }
    if (
      this.selectedPersonId !== null &&
      !this.people.some((person) => person.id === this.selectedPersonId)
    ) {
      this.selectedPersonId = this.people[0]?.id ?? null
    }
    await this.loadLatestReports()
  }

  private async loadUnmatchedAuthors(): Promise<void> {
    try {
      this.unmatchedAuthors = await fetchUnmatchedAuthors()
    } catch {
      this.unmatchedAuthors = []
    }
  }

  private async loadDashboard(): Promise<void> {
    try {
      this.dashboard = await fetchDashboard()
    } catch {
      this.dashboard = null
    }
  }

  private async loadProjects(): Promise<void> {
    try {
      const response = await fetchProjects()
      this.projects = response.projects
      if (this.selectedProjectName === null && this.projects.length > 0) {
        this.selectedProjectName = this.projects[0].name
      }
      if (
        this.selectedProjectName !== null &&
        !this.projects.some((project) => project.name === this.selectedProjectName)
      ) {
        this.selectedProjectName = this.projects[0]?.name ?? null
      }
      if (!this.isCreatingProject) {
        this.syncProjectDraft()
      }
    } catch {
      this.projects = []
      this.selectedProjectName = null
    }
  }

  private async loadLatestReports(): Promise<void> {
    if (this.selectedPersonId === null) {
      this.latestReports = null
      return
    }
    try {
      this.latestReports = await fetchLatestReports(this.selectedPersonId)
    } catch {
      this.latestReports = null
    }
  }

  private async loadPersonTrends(): Promise<void> {
    if (this.selectedPersonId === null) {
      this.personTrends = null
      return
    }
    try {
      this.personTrends = await fetchPersonTrends(this.selectedPersonId)
    } catch {
      this.personTrends = null
    }
  }

  private render(statusLine: string): void {
    this.root.innerHTML = `
      <div class="layout">
        <header class="header">
          <div>
            <h1>Reviewer</h1>
            <p class="status-line">${escapeHtml(statusLine)}</p>
          </div>
          <div class="header-actions">
            <button id="toggle-unmatched" class="secondary" type="button" ${this.unmatchedAuthors.length === 0 ? 'disabled' : ''}>
              未歸戶 (${this.unmatchedAuthors.length})
            </button>
          </div>
        </header>
        ${this.bannerMessage ? `<div class="banner${this.bannerIsError ? ' error' : ''}" role="status"><span class="banner-text">${escapeHtml(this.bannerMessage)}</span><button type="button" class="banner-dismiss" id="banner-dismiss" aria-label="關閉提示">×</button></div>` : ''}
        ${this.showUnmatchedPanel ? this.renderUnmatchedPanel() : ''}
        <div class="main">
          <aside class="sidebar">
            <nav class="sidebar-nav" aria-label="主要導覽">
              <button type="button" class="nav-item ${this.appView === 'dashboard' ? 'active' : ''}" data-nav="dashboard">
                控制台
              </button>
              <button type="button" class="nav-item ${this.appView === 'projects' ? 'active' : ''}" data-nav="projects">
                專案設定
              </button>
              <button type="button" class="nav-item ${this.appView === 'people' ? 'active' : ''}" data-nav="people">
                人員設定
              </button>
              <button type="button" class="nav-item ${this.appView === 'reports' ? 'active' : ''}" data-nav="reports">
                報告閱讀器
              </button>
              <button type="button" class="nav-item ${this.appView === 'mr-inbox' ? 'active' : ''}" data-nav="mr-inbox">
                MR 收件匣
                ${this.mrDraftBadge()}
              </button>
            </nav>
            ${this.appView === 'reports' ? `<h2>人員</h2>${this.renderPeopleList()}` : ''}
          </aside>
          <section class="content">
            ${
              this.appView === 'dashboard'
                ? this.renderDashboard()
                : this.appView === 'projects'
                  ? this.renderProjectSettings()
                  : this.appView === 'people'
                    ? this.renderPeopleSettings()
                    : this.appView === 'mr-inbox'
                      ? this.renderMrInbox()
                      : this.renderContent()
            }
          </section>
        </div>
      </div>
    `

    this.root.querySelector('#banner-dismiss')?.addEventListener('click', () => {
      this.bannerMessage = null
      this.bannerIsError = false
      void this.renderWithStatus()
    })

    this.root.querySelectorAll('[data-nav]').forEach((element) => {
      element.addEventListener('click', () => {
        const view = (element as HTMLElement).dataset.nav as AppView
        void this.switchAppView(view)
      })
    })

    this.root.querySelector('#dashboard-run')?.addEventListener('click', () => {
      void this.handleRunAll()
    })

    this.root.querySelectorAll('[data-recent-person]').forEach((element) => {
      element.addEventListener('click', () => {
        const personId = Number((element as HTMLElement).dataset.recentPerson)
        void this.openPersonReport(personId)
      })
    })

    this.root.querySelector('#reload-projects')?.addEventListener('click', () => {
      void this.handleReloadProjects()
    })

    this.root.querySelector('#toggle-unmatched')?.addEventListener('click', () => {
      this.showUnmatchedPanel = !this.showUnmatchedPanel
      void this.renderWithStatus()
    })

    this.root.querySelector('#close-unmatched')?.addEventListener('click', () => {
      this.showUnmatchedPanel = false
      void this.renderWithStatus()
    })

    this.bindPeopleSettingsEvents()

    this.root.querySelectorAll('[data-bind-existing]').forEach((element) => {
      element.addEventListener('click', () => {
        const authorId = Number((element as HTMLElement).dataset.bindExisting)
        const select = this.root.querySelector(
          `[data-person-select="${authorId}"]`,
        ) as HTMLSelectElement | null
        const personId = Number(select?.value)
        if (!personId) {
          return
        }
        void this.handleBindExisting(authorId, personId)
      })
    })

    this.root.querySelectorAll('[data-bind-new]').forEach((element) => {
      element.addEventListener('click', () => {
        const authorId = Number((element as HTMLElement).dataset.bindNew)
        const input = this.root.querySelector(
          `[data-new-name="${authorId}"]`,
        ) as HTMLInputElement | null
        const displayName = input?.value.trim()
        if (!displayName) {
          return
        }
        void this.handleBindNew(authorId, displayName)
      })
    })

    this.root.querySelectorAll('[data-person-id]').forEach((element) => {
      element.addEventListener('click', () => {
        const personId = Number((element as HTMLElement).dataset.personId)
        void this.selectPerson(personId)
      })
    })

    this.root.querySelector('#mark-read')?.addEventListener('click', () => {
      void this.handleMarkRead()
    })

    this.root.querySelectorAll('[data-pending-item-id]').forEach((element) => {
      element.addEventListener('change', (event) => {
        if (!(event.target as HTMLInputElement).checked) {
          return
        }
        const itemId = Number((element as HTMLElement).dataset.pendingItemId)
        void this.handleResolvePendingItem(itemId)
      })
    })

    this.root.querySelectorAll('[data-view-tab]').forEach((element) => {
      element.addEventListener('click', () => {
        const tab = (element as HTMLElement).dataset.viewTab as 'weekly' | 'trends'
        void this.switchViewTab(tab)
      })
    })

    this.root.querySelectorAll('[data-project-name]').forEach((element) => {
      element.addEventListener('click', () => {
        this.isCreatingProject = false
        this.selectedProjectName = (element as HTMLElement).dataset.projectName ?? null
        this.syncProjectDraft()
        void this.renderWithStatus()
      })
    })

    this.root.querySelectorAll('[data-project-run]').forEach((element) => {
      element.addEventListener('click', (event) => {
        event.stopPropagation()
        const projectName = (element as HTMLElement).dataset.projectRun
        if (projectName) {
          void this.handleProjectRun(projectName)
        }
      })
    })

    this.root.querySelector('#project-run-all')?.addEventListener('click', () => {
      void this.handleRunAll()
    })

    this.root.querySelectorAll('[data-source-type]').forEach((element) => {
      element.addEventListener('click', () => {
        const sourceType = (element as HTMLElement).dataset.sourceType as SourceType
        if (!this.projectDraft) {
          return
        }
        this.projectDraft.source_type = sourceType
        void this.renderWithStatus()
      })
    })

    this.root.querySelector('#project-add')?.addEventListener('click', () => {
      this.isCreatingProject = true
      this.selectedProjectName = null
      this.projectDraft = this.emptyProjectDraft()
      void this.renderWithStatus()
    })

    this.root.querySelector('#project-remove')?.addEventListener('click', () => {
      void this.handleProjectDelete()
    })

    this.root.querySelector('#project-save')?.addEventListener('click', () => {
      void this.handleProjectSave()
    })

    this.root.querySelector('#project-cancel')?.addEventListener('click', () => {
      this.isCreatingProject = false
      if (this.projects.length > 0) {
        this.selectedProjectName = this.projects[0]?.name ?? null
      }
      this.syncProjectDraft()
      void this.renderWithStatus()
    })

    this.root.querySelector('#project-engineer-add')?.addEventListener('click', () => {
      this.bannerMessage = '工程師對應請至人員設定管理 identity'
      this.bannerIsError = false
      void this.renderWithStatus()
    })

    this.root.querySelectorAll('[data-mr-status]').forEach((element) => {
      element.addEventListener('click', () => {
        const status = (element as HTMLElement).dataset.mrStatus as MrReviewStatus
        void this.switchMrStatusFilter(status)
      })
    })

    this.root.querySelectorAll('[data-mr-review-id]').forEach((element) => {
      element.addEventListener('click', () => {
        const id = Number((element as HTMLElement).dataset.mrReviewId)
        this.selectMrReview(id)
        void this.renderWithStatus()
      })
    })

    this.root.querySelector('#mr-editor')?.addEventListener('input', (event) => {
      this.mrEditorBody = (event.target as HTMLTextAreaElement).value
      this.mrEditorDirty = true
    })

    this.root.querySelector('#mr-save')?.addEventListener('click', () => {
      void this.handleMrSave()
    })

    this.root.querySelector('#mr-publish')?.addEventListener('click', () => {
      void this.handleMrPublish()
    })

    this.root.querySelector('#mr-ignore')?.addEventListener('click', () => {
      void this.handleMrIgnore()
    })

    this.root.querySelector('#mr-chat-send')?.addEventListener('click', () => {
      void this.handleMrAgentTurn()
    })

    this.root.querySelector('#mr-chat-input')?.addEventListener('keydown', (event) => {
      const keyEvent = event as KeyboardEvent
      if (keyEvent.key === 'Enter' && !keyEvent.shiftKey) {
        keyEvent.preventDefault()
        void this.handleMrAgentTurn()
      }
    })

    this.root.querySelector('#dashboard-mr-drafts')?.addEventListener('click', () => {
      void this.switchAppView('mr-inbox')
    })

    this.root.querySelector('#schedule-save')?.addEventListener('click', () => {
      void this.handleScheduleSave()
    })

    this.root.querySelector('#project-mr-scan')?.addEventListener('click', () => {
      const project = this.selectedProject
      if (project) {
        void this.handleMrScan(project.id)
      }
    })

    this.root.querySelector('#project-mr-scan-force')?.addEventListener('click', () => {
      const project = this.selectedProject
      if (project) {
        void this.handleMrScan(project.id, true)
      }
    })
  }

  private renderDashboard(): string {
    const dashboard = this.dashboard
    const lastRunLine = dashboard?.last_run
      ? `上次執行 ${formatTimestamp(dashboard.last_run.started_at)}${formatDurationSuffix(dashboard.last_run.duration_sec)}`
      : '尚無執行紀錄'

    const stats = dashboard?.stats ?? {
      project_count: 0,
      person_count: 0,
      unread_count: 0,
      pending_count: 0,
      mr_draft_count: 0,
    }

    const recentRows =
      dashboard?.recent_reports.length === 0
        ? '<p class="empty dashboard-empty">尚無報告</p>'
        : (dashboard?.recent_reports ?? [])
            .map((report) => {
              const status = report.is_read
                ? '<span class="recent-status read">已讀</span>'
                : `<span class="recent-status pending">${report.pending_count > 0 ? `⚠ ${report.pending_count}` : ''}</span>`
              const dotClass = report.is_read ? 'read' : 'unread'
              const rowClass = report.is_read ? 'recent-row read' : 'recent-row'
              return `<button type="button" class="${rowClass}" data-recent-person="${report.person_id}">
                <span class="recent-label">
                  <span class="recent-dot ${dotClass}" aria-hidden="true"></span>
                  ${escapeHtml(report.person_name)}
                  <span class="recent-project">${escapeHtml(report.project_name)}</span>
                </span>
                ${status}
              </button>`
            })
            .join('')

    const schedule = dashboard?.schedule
    const scheduleStatus = schedule?.enabled
      ? '<div class="schedule-status active"><span aria-hidden="true">✓</span> 排程器運行中</div>'
      : '<div class="schedule-status inactive">排程已停用</div>'

    return `<div class="dashboard">
      <div class="dashboard-header">
        <div>
          <h2 class="dashboard-title">控制台</h2>
          <p class="dashboard-subtitle">${escapeHtml(lastRunLine)}</p>
        </div>
        <button id="dashboard-run" class="dashboard-run" type="button" ${this.activeRunId || this.reloading ? 'disabled' : ''}>
          ${this.activeRunId ? '執行中…' : '▶ 立即執行'}
        </button>
      </div>

      <div class="dashboard-stats">
        <article class="stat-card">
          <div class="stat-label">專案</div>
          <div class="stat-value">${stats.project_count}</div>
        </article>
        <article class="stat-card">
          <div class="stat-label">工程師</div>
          <div class="stat-value">${stats.person_count}</div>
        </article>
        <article class="stat-card">
          <div class="stat-label">未讀報告</div>
          <div class="stat-value accent-info">${stats.unread_count}</div>
        </article>
        <article class="stat-card">
          <div class="stat-label">待確認</div>
          <div class="stat-value accent-warning">${stats.pending_count}</div>
        </article>
        <button type="button" id="dashboard-mr-drafts" class="stat-card stat-card-button">
          <div class="stat-label">MR 草稿</div>
          <div class="stat-value accent-mr">${stats.mr_draft_count}</div>
        </button>
      </div>

      <div class="dashboard-panels">
        <section class="dashboard-panel">
          <h3 class="panel-title">最近報告</h3>
          <div class="recent-list">${recentRows}</div>
        </section>
        <section class="dashboard-panel">
          <h3 class="panel-title">排程</h3>
          <div class="schedule-section">
            <div class="schedule-section-title">週報（軌道 1）</div>
            <div class="schedule-row"><span aria-hidden="true">📅</span> ${escapeHtml(schedule?.label ?? '—')}</div>
            <div class="schedule-row muted">
              <span aria-hidden="true">🕒</span>
              ${schedule?.next_run_at ? `下次 ${escapeHtml(schedule.next_run_at)}` : '無下次排程'}
            </div>
            ${scheduleStatus}
          </div>
          <div class="schedule-section schedule-section-mr">
            <div class="schedule-section-title">MR 輪詢（軌道 2）</div>
            <div class="schedule-row">
              <span aria-hidden="true">🔁</span>
              ${escapeHtml(schedule?.mr_poll_label ?? '—')}
              ${schedule?.mr_poll_interval_min && schedule.mr_poll_interval_min <= 0 ? '（已停用）' : ''}
            </div>
            <div class="schedule-mr-poll-edit">
              <label class="schedule-mr-poll-label" for="schedule-mr-poll-interval">間隔（分鐘）</label>
              <input
                id="schedule-mr-poll-interval"
                class="schedule-mr-poll-input"
                type="number"
                min="1"
                step="1"
                value="${schedule?.mr_poll_interval_min ?? 60}"
              />
              <button id="schedule-save" class="schedule-save-btn" type="button" ${this.scheduleSaving ? 'disabled' : ''}>
                ${this.scheduleSaving ? '儲存中…' : '儲存'}
              </button>
            </div>
            <p class="schedule-field-hint">≥60 時須為 60 的倍數（如 60、120）。變更後需重啟 reviewer-server 才會套用新 cron。</p>
          </div>
        </section>
      </div>
    </div>`
  }

  private renderProjectListItem(project: ProjectListItem): string {
    const active =
      !this.isCreatingProject && project.name === this.selectedProjectName ? ' active' : ''
    const runState = this.getProjectRunState(project.name)
    const running = runState === 'queued' || runState === 'running'
    const icon = project.source_type === 'gitlab' ? 'gitlab' : 'folder'
    const dateLabel = project.last_report_date
      ? formatReportDateShort(project.last_report_date)
      : ''
    const runDisabled = this.activeRunId !== null || this.reloading

    const trailing = running
      ? `<span class="project-list-running">
          <span class="project-list-spinner" aria-hidden="true"></span>
          <span class="project-list-elapsed">${formatRunElapsed(this.activeRun?.started_at ?? null, this.runElapsedTick)}</span>
        </span>`
      : `<span class="project-list-date">${escapeHtml(dateLabel)}</span>
         <button type="button" class="project-list-run" data-project-run="${escapeHtml(project.name)}" ${runDisabled ? 'disabled' : ''}>▶ 執行</button>`

    return `<div class="project-list-item${active}${running ? ' running' : ''}">
      <button type="button" class="project-list-main" data-project-name="${escapeHtml(project.name)}">
        <span class="project-list-icon ${icon}" aria-hidden="true"></span>
        <span class="project-list-name">${escapeHtml(project.name)}</span>
      </button>
      <span class="project-list-trailing">${trailing}</span>
    </div>`
  }

  private getProjectRunState(projectName: string): string | null {
    return this.activeRun?.projects.find((project) => project.name === projectName)?.state ?? null
  }

  private isProjectRunning(projectName: string): boolean {
    const state = this.getProjectRunState(projectName)
    return state === 'queued' || state === 'running'
  }

  private renderProjectSettings(): string {
    const draft = this.projectDraft ?? this.emptyProjectDraft()
    const selected = this.selectedProject
    const listItems = this.projects.map((project) => this.renderProjectListItem(project)).join('')

    const sourceGitlab = draft.source_type === 'gitlab' ? ' active' : ''
    const sourceLocal = draft.source_type === 'local' ? ' active' : ''
    const headerIcon = draft.source_type === 'gitlab' ? 'gitlab' : 'folder'
    const title = draft.isNew ? '新增專案' : escapeHtml(draft.name)
    const healthNote =
      !draft.isNew && selected?.health === 'unhealthy' && selected.health_reason
        ? `<p class="project-health-warning">狀態異常：${escapeHtml(selected.health_reason)}</p>`
        : ''

    const engineerRows =
      selected && selected.engineers.length > 0
        ? selected.engineers
            .map((engineer) => {
              const initial = engineer.display_name.trim().charAt(0).toUpperCase() || '?'
              const username = engineer.gitlab_username ?? '—'
              return `<div class="project-engineer-row">
                <span class="project-engineer-avatar" aria-hidden="true">${escapeHtml(initial)}</span>
                <span class="project-engineer-username">${escapeHtml(username)}</span>
                <span class="project-engineer-arrow" aria-hidden="true">→</span>
                <span>${escapeHtml(engineer.display_name)}</span>
              </div>`
            })
            .join('')
        : '<p class="project-engineers-empty">尚無工程師對應（執行 review 後會依 commit 歸戶）</p>'

    const gitlabFields =
      draft.source_type === 'gitlab'
        ? `<div class="project-field">
            <label for="project-git-remote">Git Remote URL <span class="required" aria-hidden="true">*</span></label>
            <input id="project-git-remote" class="project-input mono" type="text" value="${escapeHtml(draft.git_remote_url)}" placeholder="git@gitlab.example.com:team/repo.git" />
            <p class="project-field-hint">用於 clone 遠端 repo，請從 GitLab 專案的「Clone」複製 SSH 或 HTTPS 網址。</p>
          </div>
          <div class="project-field">
            <label for="project-default-branches">常駐分支 <span class="required" aria-hidden="true">*</span></label>
            <input id="project-default-branches" class="project-input mono" type="text" value="${escapeHtml(draft.default_branches)}" placeholder="main, develop" />
            <p class="project-field-hint">啟動時會為這些分支建立 worktree，週報預設看第一個分支。</p>
          </div>
          <div class="project-field project-field-mr-gates">
            <label for="project-mr-skip-labels">MR 排除標籤</label>
            <input id="project-mr-skip-labels" class="project-input mono" type="text" value="${escapeHtml(draft.mr_review_skip_labels)}" placeholder="wip, do-not-review, no-ai-review" />
            <p class="project-field-hint">逗號分隔。帶有任一標籤的 MR 不會進入 AI review；留空表示不排除任何標籤。</p>
          </div>
          <div class="project-field project-field-mr-gates">
            <label for="project-mr-require-label">MR 必備標籤（可選）</label>
            <input id="project-mr-require-label" class="project-input mono" type="text" value="${escapeHtml(draft.mr_review_require_label)}" placeholder="ready-for-review" />
            <p class="project-field-hint">設定後，只有帶此標籤的 MR 才會被掃描（opt-in 模式）。留空則不啟用。</p>
          </div>`
        : ''

    const nameField = draft.isNew
      ? `<div class="project-field">
          <label for="project-name">專案名稱 <span class="required" aria-hidden="true">*</span></label>
          <input id="project-name" class="project-input" type="text" value="${escapeHtml(draft.name)}" placeholder="game-backend" />
        </div>`
      : ''

    const selectedRunning = draft.isNew ? false : this.isProjectRunning(draft.name)
    const runningBadge = selectedRunning
      ? '<span class="project-settings-running-badge"><span class="project-list-spinner" aria-hidden="true"></span>執行中</span>'
      : ''

    return `<div class="project-settings">
      <div class="project-settings-toolbar">
        <h2 class="project-settings-page-title">專案設定</h2>
        <button id="reload-projects" class="project-settings-reload" type="button" ${this.reloading || this.activeRunId ? 'disabled' : ''}>
          ${this.reloading ? '載入中…' : '重新載入'}
        </button>
      </div>
      <h2 class="sr-only">專案設定頁，左側為專案清單，右側為選定專案的詳細設定</h2>
      <div class="project-settings-shell">
        <aside class="project-settings-list">
          <div class="project-settings-list-header">
            <span>專案</span>
            <div class="project-settings-list-actions">
              <button id="project-run-all" class="project-settings-run-all" type="button" ${this.activeRunId || this.reloading ? 'disabled' : ''}>
                ▶ 全部
              </button>
              <button id="project-add" class="project-settings-add" type="button" aria-label="新增專案">＋</button>
            </div>
          </div>
          <div class="project-settings-list-items">
            ${listItems || '<p class="project-list-empty">尚無專案</p>'}
          </div>
          <p class="project-list-hint">
            <span class="project-list-hint-icon" aria-hidden="true">ⓘ</span>
            滑過專案顯示「執行」，單獨跑完即更新該專案報告
          </p>
        </aside>
        <div class="project-settings-detail">
          <div class="project-settings-detail-header">
            <div class="project-settings-title">
              <span class="project-list-icon ${headerIcon}" aria-hidden="true"></span>
              <h2>${title}</h2>
              ${runningBadge}
            </div>
            ${
              draft.isNew
                ? ''
                : `<div class="project-settings-detail-actions">
                    ${selected && selected.is_git_repo ? `<button id="project-mr-scan" class="project-settings-mr-scan" type="button" ${this.activeRunId || this.reloading ? 'disabled' : ''}>掃描 MR</button><button id="project-mr-scan-force" class="project-settings-mr-scan-force" type="button" ${this.activeRunId || this.reloading ? 'disabled' : ''} title="略過收件匣草稿閘門，強制重掃">強制重掃</button>` : ''}
                    <button id="project-remove" class="project-settings-remove" type="button" ${this.activeRunId || this.reloading ? 'disabled' : ''}>移除</button>
                  </div>`
            }
          </div>
          ${healthNote}
          ${nameField}
          <div class="project-field">
            <label>來源類型</label>
            <div class="project-source-types">
              <button type="button" class="project-source-pill${sourceGitlab}" data-source-type="gitlab">GitLab</button>
              <button type="button" class="project-source-pill${sourceLocal}" data-source-type="local">本地</button>
            </div>
          </div>
          ${gitlabFields}
          <div class="project-field">
            <label for="project-repo-path">儲存路徑 <span class="required" aria-hidden="true">*</span></label>
            <input id="project-repo-path" class="project-input mono" type="text" value="${escapeHtml(draft.repo_path)}" placeholder="game-backend" />
            <p class="project-field-hint">簡短名稱即可（例 <code>game-backend</code>），會對應到伺服器的 <code>repos/</code> 目錄；若已有固定路徑可填絕對路徑。</p>
          </div>
          <div class="project-field">
            <div class="project-field-header">
              <label>工程師對應 <span class="muted">(GitLab username → 顯示名，唯讀)</span></label>
              <button id="project-engineer-add" class="project-engineer-add" type="button" aria-label="新增工程師對應">＋</button>
            </div>
            <div class="project-engineer-list">${engineerRows}</div>
          </div>
          <div class="project-settings-actions">
            <button id="project-cancel" class="project-settings-cancel" type="button">取消</button>
            <button id="project-save" class="project-settings-save" type="button">儲存</button>
          </div>
        </div>
      </div>
    </div>`
  }

  private get selectedProject(): ProjectListItem | null {
    if (this.selectedProjectName === null) {
      return null
    }
    return this.projects.find((project) => project.name === this.selectedProjectName) ?? null
  }

  private renderPeopleSettings(): string {
    const listItems = this.people
      .map((person) => {
        const active =
          !this.isCreatingPerson && person.id === this.selectedSettingsPersonId ? ' active' : ''
        return `<button type="button" class="people-settings-list-item${active}" data-settings-person-id="${person.id}">
          <span class="people-settings-list-name">${escapeHtml(person.display_name)}</span>
          <span class="people-settings-list-meta">${person.identity_count} identities</span>
        </button>`
      })
      .join('')

    if (this.isCreatingPerson) {
      return `<div class="people-settings">
        <div class="people-settings-toolbar">
          <h2 class="people-settings-page-title">人員設定</h2>
        </div>
        <div class="people-settings-shell">
          <aside class="people-settings-list">
            <div class="people-settings-list-header">
              <span>人員</span>
              <button id="people-add" class="people-settings-add" type="button" aria-label="新增人員">＋</button>
            </div>
            <div class="people-settings-list-items">
              ${listItems || '<p class="people-list-empty">尚無人員</p>'}
            </div>
          </aside>
          <div class="people-settings-detail">
            <div class="people-settings-detail-header">
              <h2>新增人員</h2>
            </div>
            <div class="people-field">
              <label for="people-new-name">顯示名 <span class="required" aria-hidden="true">*</span></label>
              <input id="people-new-name" class="people-input" type="text" value="${escapeHtml(this.newPersonName)}" placeholder="Alice Chen" />
            </div>
            <div class="people-settings-actions">
              <button id="people-create-cancel" class="people-settings-cancel" type="button">取消</button>
              <button id="people-create-save" class="people-settings-save" type="button" ${this.peopleSettingsSaving ? 'disabled' : ''}>建立</button>
            </div>
          </div>
        </div>
      </div>`
    }

    const detail = this.personDetail
    if (!detail) {
      return `<div class="people-settings">
        <div class="people-settings-toolbar">
          <h2 class="people-settings-page-title">人員設定</h2>
        </div>
        <div class="people-settings-shell">
          <aside class="people-settings-list">
            <div class="people-settings-list-header">
              <span>人員</span>
              <button id="people-add" class="people-settings-add" type="button" aria-label="新增人員">＋</button>
            </div>
            <div class="people-settings-list-items">
              ${listItems || '<p class="people-list-empty">尚無人員</p>'}
            </div>
          </aside>
          <div class="people-settings-detail">
            <p class="people-settings-empty">選擇左側人員，或新增一位人員。</p>
          </div>
        </div>
      </div>`
    }

    const identityRows =
      detail.identities.length > 0
        ? detail.identities
            .map(
              (identity) => `<div class="people-identity-row">
                <span class="people-identity-kind">${escapeHtml(identity.kind)}</span>
                <span class="people-identity-value mono">${escapeHtml(identity.value)}</span>
                ${identity.label ? `<span class="people-identity-label">${escapeHtml(identity.label)}</span>` : ''}
                <button type="button" class="people-identity-remove" data-unbind-identity="${identity.id}" ${this.peopleSettingsSaving ? 'disabled' : ''}>移除</button>
              </div>`,
            )
            .join('')
        : '<p class="people-identities-empty">尚無 identity；未歸戶 commit 會進入佇列。</p>'

    const projectRows =
      detail.projects.length > 0
        ? detail.projects
            .map(
              (project) =>
                `<li class="people-project-item">${escapeHtml(project.name)}</li>`,
            )
            .join('')
        : '<p class="people-projects-empty">尚無參與專案（來自報告或 participation）</p>'

    const kindOptions = IDENTITY_KINDS.map(
      (kind) =>
        `<option value="${kind.value}" ${this.identityDraftKind === kind.value ? 'selected' : ''}>${kind.label}</option>`,
    ).join('')

    return `<div class="people-settings">
      <div class="people-settings-toolbar">
        <h2 class="people-settings-page-title">人員設定</h2>
      </div>
      <h2 class="sr-only">人員設定頁，左側為人員清單，右側為選定人員的詳細設定</h2>
      <div class="people-settings-shell">
        <aside class="people-settings-list">
          <div class="people-settings-list-header">
            <span>人員</span>
            <button id="people-add" class="people-settings-add" type="button" aria-label="新增人員">＋</button>
          </div>
          <div class="people-settings-list-items">
            ${listItems || '<p class="people-list-empty">尚無人員</p>'}
          </div>
        </aside>
        <div class="people-settings-detail">
          <div class="people-settings-detail-header">
            <h2>${escapeHtml(detail.display_name)}</h2>
          </div>
          <div class="people-field">
            <label for="people-display-name">顯示名</label>
            <div class="people-display-name-row">
              <input id="people-display-name" class="people-input" type="text" value="${escapeHtml(detail.display_name)}" />
              <button id="people-rename-save" class="people-settings-save" type="button" ${this.peopleSettingsSaving ? 'disabled' : ''}>儲存</button>
            </div>
            <p class="people-field-hint">更名會同步 rename <code>reports/_people/{顯示名}/</code>；專案層報告目錄不會搬移。</p>
          </div>
          <div class="people-field">
            <label>Identities</label>
            <div class="people-identity-list">${identityRows}</div>
            <div class="people-identity-add">
              <select id="people-identity-kind" class="people-input" aria-label="identity kind">
                ${kindOptions}
              </select>
              <input id="people-identity-value" class="people-input mono" type="text" value="${escapeHtml(this.identityDraftValue)}" placeholder="value" />
              <button id="people-identity-bind" class="people-settings-save" type="button" ${this.peopleSettingsSaving ? 'disabled' : ''}>新增</button>
            </div>
          </div>
          <div class="people-field">
            <label>參與專案 <span class="muted">(唯讀)</span></label>
            ${
              detail.projects.length > 0
                ? `<ul class="people-project-list">${projectRows}</ul>`
                : projectRows
            }
          </div>
        </div>
      </div>
    </div>`
  }

  private bindPeopleSettingsEvents(): void {
    this.root.querySelector('#people-add')?.addEventListener('click', () => {
      this.isCreatingPerson = true
      this.newPersonName = ''
      this.personDetail = null
      void this.renderWithStatus()
    })

    this.root.querySelectorAll('[data-settings-person-id]').forEach((element) => {
      element.addEventListener('click', () => {
        const personId = Number((element as HTMLElement).dataset.settingsPersonId)
        void this.selectSettingsPerson(personId)
      })
    })

    this.root.querySelector('#people-create-cancel')?.addEventListener('click', () => {
      this.isCreatingPerson = false
      this.newPersonName = ''
      void this.switchAppView('people')
    })

    this.root.querySelector('#people-create-save')?.addEventListener('click', () => {
      const input = this.root.querySelector('#people-new-name') as HTMLInputElement | null
      void this.handleCreatePerson(input?.value ?? '')
    })

    this.root.querySelector('#people-rename-save')?.addEventListener('click', () => {
      const input = this.root.querySelector('#people-display-name') as HTMLInputElement | null
      void this.handleRenamePerson(input?.value ?? '')
    })

    this.root.querySelector('#people-identity-kind')?.addEventListener('change', (event) => {
      this.identityDraftKind = (event.target as HTMLSelectElement).value as IdentityKind
    })

    this.root.querySelector('#people-identity-value')?.addEventListener('input', (event) => {
      this.identityDraftValue = (event.target as HTMLInputElement).value
    })

    this.root.querySelector('#people-identity-bind')?.addEventListener('click', () => {
      const kind = (this.root.querySelector('#people-identity-kind') as HTMLSelectElement | null)
        ?.value as IdentityKind | undefined
      const value = (this.root.querySelector('#people-identity-value') as HTMLInputElement | null)
        ?.value
      void this.handleBindSettingsIdentity(kind ?? 'git_email', value ?? '')
    })

    this.root.querySelectorAll('[data-unbind-identity]').forEach((element) => {
      element.addEventListener('click', () => {
        const identityId = Number((element as HTMLElement).dataset.unbindIdentity)
        void this.handleUnbindIdentity(identityId)
      })
    })
  }

  private async loadPersonDetail(personId: number): Promise<void> {
    try {
      this.personDetail = await fetchPersonDetail(personId)
    } catch (error) {
      this.personDetail = null
      this.bannerMessage = error instanceof Error ? error.message : '無法載入人員詳情'
      this.bannerIsError = true
    }
  }

  private async selectSettingsPerson(personId: number): Promise<void> {
    this.isCreatingPerson = false
    this.selectedSettingsPersonId = personId
    this.identityDraftValue = ''
    this.identityDraftKind = 'git_email'
    await this.loadPersonDetail(personId)
    this.render(await this.statusLine())
  }

  private async handleCreatePerson(displayName: string): Promise<void> {
    const trimmed = displayName.trim()
    if (!trimmed || this.peopleSettingsSaving) {
      return
    }
    this.peopleSettingsSaving = true
    await this.renderWithStatus()
    try {
      const created = await createPerson(trimmed)
      await this.loadPeople()
      this.isCreatingPerson = false
      this.newPersonName = ''
      this.selectedSettingsPersonId = created.id
      await this.loadPersonDetail(created.id)
      this.bannerMessage = `已建立 ${created.display_name}`
      this.bannerIsError = false
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '建立人員失敗'
      this.bannerIsError = true
    } finally {
      this.peopleSettingsSaving = false
      await this.renderWithStatus()
    }
  }

  private async handleRenamePerson(displayName: string): Promise<void> {
    if (this.selectedSettingsPersonId === null || this.peopleSettingsSaving) {
      return
    }
    const trimmed = displayName.trim()
    if (!trimmed) {
      this.bannerMessage = '顯示名不可為空'
      this.bannerIsError = true
      await this.renderWithStatus()
      return
    }
    this.peopleSettingsSaving = true
    await this.renderWithStatus()
    try {
      this.personDetail = await renamePerson(this.selectedSettingsPersonId, trimmed)
      await this.loadPeople()
      this.bannerMessage = '已更新顯示名'
      this.bannerIsError = false
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '更名失敗'
      this.bannerIsError = true
    } finally {
      this.peopleSettingsSaving = false
      await this.renderWithStatus()
    }
  }

  private async handleBindSettingsIdentity(kind: IdentityKind, value: string): Promise<void> {
    if (this.selectedSettingsPersonId === null || this.peopleSettingsSaving) {
      return
    }
    const trimmed = value.trim()
    if (!trimmed) {
      this.bannerMessage = 'identity value 不可為空'
      this.bannerIsError = true
      await this.renderWithStatus()
      return
    }
    if (this.personDetail?.identities.some((item) => item.kind === kind && item.value === trimmed)) {
      this.bannerMessage = '此 identity 已綁定'
      this.bannerIsError = false
      return
    }
    this.peopleSettingsSaving = true
    await this.renderWithStatus()
    try {
      await bindIdentity(this.selectedSettingsPersonId, kind, trimmed)
      this.identityDraftValue = ''
      await Promise.all([
        this.loadPersonDetail(this.selectedSettingsPersonId),
        this.loadPeople(),
        this.loadUnmatchedAuthors(),
      ])
      this.bannerMessage = '已新增 identity'
      this.bannerIsError = false
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '綁定失敗'
      this.bannerIsError = true
    } finally {
      this.peopleSettingsSaving = false
      await this.renderWithStatus()
    }
  }

  private async handleUnbindIdentity(identityId: number): Promise<void> {
    if (this.selectedSettingsPersonId === null || this.peopleSettingsSaving) {
      return
    }
    this.peopleSettingsSaving = true
    await this.renderWithStatus()
    try {
      await unbindIdentity(this.selectedSettingsPersonId, identityId)
      await Promise.all([
        this.loadPersonDetail(this.selectedSettingsPersonId),
        this.loadPeople(),
        this.loadUnmatchedAuthors(),
      ])
      this.bannerMessage = '已移除 identity'
      this.bannerIsError = false
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '移除失敗'
      this.bannerIsError = true
    } finally {
      this.peopleSettingsSaving = false
      await this.renderWithStatus()
    }
  }

  private emptyProjectDraft(): ProjectDraft {
    return {
      name: '',
      source_type: 'gitlab',
      repo_path: '',
      git_remote_url: '',
      default_branches: 'main',
      mr_review_skip_labels: DEFAULT_MR_SKIP_LABELS.join(', '),
      mr_review_require_label: '',
      isNew: true,
    }
  }

  private syncProjectDraft(): void {
    const selected = this.selectedProject
    if (!selected) {
      this.projectDraft = this.emptyProjectDraft()
      this.isCreatingProject = true
      return
    }
    this.projectDraft = {
      name: selected.name,
      source_type: selected.source_type,
      repo_path: selected.repo_path,
      git_remote_url: selected.git_remote_url ?? '',
      default_branches: (selected.default_branches.length > 0
        ? selected.default_branches
        : selected.default_branch
          ? [selected.default_branch]
          : ['main']
      ).join(', '),
      mr_review_skip_labels: (selected.mr_review_skip_labels.length > 0
        ? selected.mr_review_skip_labels
        : DEFAULT_MR_SKIP_LABELS
      ).join(', '),
      mr_review_require_label: selected.mr_review_require_label ?? '',
      isNew: false,
    }
  }

  private readProjectDraftFromForm(): ProjectDraft | null {
    const draft = this.projectDraft
    if (!draft) {
      return null
    }
    const nameInput = this.root.querySelector('#project-name') as HTMLInputElement | null
    const repoPathInput = this.root.querySelector('#project-repo-path') as HTMLInputElement | null
    const gitRemoteInput = this.root.querySelector('#project-git-remote') as HTMLInputElement | null
    const branchesInput = this.root.querySelector('#project-default-branches') as HTMLInputElement | null
    const skipLabelsInput = this.root.querySelector('#project-mr-skip-labels') as HTMLInputElement | null
    const requireLabelInput = this.root.querySelector('#project-mr-require-label') as HTMLInputElement | null

    return {
      ...draft,
      name: (nameInput?.value ?? draft.name).trim(),
      repo_path: (repoPathInput?.value ?? draft.repo_path).trim(),
      git_remote_url: (gitRemoteInput?.value ?? draft.git_remote_url).trim(),
      default_branches: (branchesInput?.value ?? draft.default_branches).trim(),
      mr_review_skip_labels: (skipLabelsInput?.value ?? draft.mr_review_skip_labels).trim(),
      mr_review_require_label: (requireLabelInput?.value ?? draft.mr_review_require_label).trim(),
    }
  }

  private parseCommaSeparatedLabels(value: string): string[] {
    return value
      .split(',')
      .map((label) => label.trim())
      .filter(Boolean)
  }

  private mrReviewGatePayload(draft: ProjectDraft) {
    if (draft.source_type !== 'gitlab') {
      return {
        mr_review_skip_labels: [] as string[],
        mr_review_require_label: null as string | null,
      }
    }
    return {
      mr_review_skip_labels: this.parseCommaSeparatedLabels(draft.mr_review_skip_labels),
      mr_review_require_label: draft.mr_review_require_label || null,
    }
  }

  private parseDefaultBranches(value: string): string[] {
    return value
      .split(',')
      .map((branch) => branch.trim())
      .filter(Boolean)
  }

  private async handleProjectSave(): Promise<void> {
    const draft = this.readProjectDraftFromForm()
    if (!draft) {
      return
    }

    const payload = {
      source_type: draft.source_type,
      repo_path: draft.repo_path,
      git_remote_url: draft.source_type === 'gitlab' ? draft.git_remote_url || null : null,
      default_branches:
        draft.source_type === 'gitlab' ? this.parseDefaultBranches(draft.default_branches) : [],
      ...this.mrReviewGatePayload(draft),
    }

    try {
      if (draft.isNew) {
        if (!draft.name) {
          throw new Error('請輸入專案名稱')
        }
        const created = await createProject({ name: draft.name, ...payload })
        this.isCreatingProject = false
        this.selectedProjectName = created.name
        this.bannerMessage = `已新增專案 ${created.name}`
        this.bannerIsError = false
      } else {
        const updated = await updateProject(draft.name, payload)
        this.selectedProjectName = updated.name
        this.bannerMessage = `已儲存專案 ${updated.name}`
        this.bannerIsError = false
      }
      await Promise.all([this.loadProjects(), this.loadDashboard()])
      this.syncProjectDraft()
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '儲存失敗'
      this.bannerIsError = true
    }
    await this.renderWithStatus()
  }

  private async handleScheduleSave(): Promise<void> {
    const input = this.root.querySelector('#schedule-mr-poll-interval') as HTMLInputElement | null
    const value = Number(input?.value)
    if (!Number.isFinite(value) || value < 1) {
      this.bannerMessage = 'MR 輪詢間隔必須為正整數（分鐘）'
      this.bannerIsError = true
      await this.renderWithStatus()
      return
    }
    this.scheduleSaving = true
    await this.renderWithStatus()
    try {
      await updateSchedule({ mr_poll_interval_min: Math.trunc(value) })
      await this.loadDashboard()
      this.bannerMessage = '排程設定已儲存（重啟服務後套用新 cron）'
      this.bannerIsError = false
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '儲存排程失敗'
      this.bannerIsError = true
    } finally {
      this.scheduleSaving = false
      await this.renderWithStatus()
    }
  }

  private async handleProjectRun(projectName?: string): Promise<void> {
    const name = projectName ?? this.projectDraft?.name
    if (!name || this.projectDraft?.isNew || this.activeRunId || this.reloading) {
      return
    }

    try {
      const response = await startProjectRun(name)
      this.activeRunId = response.run_id
      this.bannerMessage = null
      this.bannerIsError = false
      this.render(`執行中 · ${name} · run #${response.run_id}`)
      this.startPolling(response.run_id)
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '無法啟動執行'
      this.bannerIsError = true
      await this.renderWithStatus()
    }
  }

  private async handleProjectDelete(): Promise<void> {
    const draft = this.projectDraft
    if (!draft || draft.isNew) {
      return
    }
    if (!window.confirm(`確定要移除專案「${draft.name}」？`)) {
      return
    }

    try {
      await deleteProject(draft.name)
      this.bannerMessage = `已移除專案 ${draft.name}`
      this.bannerIsError = false
      this.isCreatingProject = false
      await Promise.all([this.loadProjects(), this.loadDashboard()])
      this.syncProjectDraft()
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '移除失敗'
      this.bannerIsError = true
    }
    await this.renderWithStatus()
  }

  private renderUnmatchedPanel(): string {
    if (this.unmatchedAuthors.length === 0) {
      return ''
    }

    const personOptions = this.people
      .map(
        (person) =>
          `<option value="${person.id}">${escapeHtml(person.display_name)}</option>`,
      )
      .join('')

    const rows = this.unmatchedAuthors
      .map((author) => {
        const projectLabel = author.project_name ?? '未知專案'
        return `<article class="unmatched-row" data-author-id="${author.id}">
          <div class="unmatched-meta">
            <strong>${escapeHtml(author.value)}</strong>
            <span>${escapeHtml(projectLabel)} · ${author.commit_count} commits</span>
          </div>
          <div class="unmatched-actions">
            <input type="text" data-new-name="${author.id}" placeholder="新顯示名稱" />
            <button type="button" data-bind-new="${author.id}">建立並綁定</button>
            <select data-person-select="${author.id}">
              <option value="">綁定到現有人員</option>
              ${personOptions}
            </select>
            <button type="button" data-bind-existing="${author.id}">綁定</button>
          </div>
        </article>`
      })
      .join('')

    return `<section class="unmatched-panel">
      <header>
        <h2>未歸戶作者</h2>
        <button id="close-unmatched" type="button">關閉</button>
      </header>
      <p class="hint">未綁定 git email 不會產出週報。綁定後請重新執行 review。</p>
      <div class="unmatched-list">${rows}</div>
    </section>`
  }

  private renderPeopleList(): string {
    if (this.people.length === 0) {
      return '<p class="empty">尚無人員資料</p>'
    }

    return `<ul class="people-list">${this.people
      .map((person) => {
        const selected = person.id === this.selectedPersonId ? 'selected' : ''
        const unread = person.unread_count > 0 ? `<span class="badge">${person.unread_count}</span>` : ''
        const pending =
          person.open_pending_count > 0
            ? `<span class="badge pending">${person.open_pending_count}</span>`
            : ''
        return `<li>
          <button type="button" class="person ${selected}" data-person-id="${person.id}">
            <span class="name">${escapeHtml(person.display_name)}</span>
            <span class="badges">${unread}${pending}</span>
          </button>
        </li>`
      })
      .join('')}</ul>`
  }

  private renderContent(): string {
    if (this.selectedPersonId === null) {
      return '<p class="empty">請從左側選擇人員</p>'
    }

    const person = this.people.find((item) => item.id === this.selectedPersonId)
    if (!person) {
      return '<p class="empty">找不到人員</p>'
    }

    const tabs = `<div class="view-tabs">
      <button type="button" class="view-tab ${this.personViewTab === 'weekly' ? 'active' : ''}" data-view-tab="weekly">本週</button>
      <button type="button" class="view-tab ${this.personViewTab === 'trends' ? 'active' : ''}" data-view-tab="trends">趨勢</button>
    </div>`

    if (this.personViewTab === 'trends') {
      return `<div class="content-header">
        <h2>${escapeHtml(person.display_name)}</h2>
      </div>
      ${tabs}
      ${this.renderTrendsContent()}`
    }

    if (!this.latestReports || this.latestReports.projects.length === 0) {
      return `<div class="content-header">
        <h2>${escapeHtml(person.display_name)}</h2>
      </div>
      ${tabs}
      <p class="empty">尚無週報</p>`
    }

    const unreadCount = this.latestReports.projects.filter((item) => !item.is_read).length
    const markReadButton =
      unreadCount > 0
        ? `<button id="mark-read" type="button">標記本週已讀 (${unreadCount})</button>`
        : '<span class="read-label">已全部已讀</span>'

    return `<div class="content-header">
      <div>
        <h2>${escapeHtml(person.display_name)}</h2>
        <p class="report-date">報告日期：${escapeHtml(this.latestReports.report_date)}</p>
      </div>
      ${markReadButton}
    </div>
    ${tabs}
    <div class="project-grid">
      ${this.latestReports.projects.map((project) => this.renderProjectCard(project)).join('')}
    </div>`
  }

  private renderTrendsContent(): string {
    const trends = this.personTrends
    const hasObservation = Boolean(trends?.long_term_observation?.trim())
    const hasTimeline = (trends?.growth_timeline.length ?? 0) > 0
    const hasPending = (trends?.historical_pending.length ?? 0) > 0

    if (!hasObservation && !hasTimeline && !hasPending) {
      return `<p class="empty">尚無長期觀察資料。可將舊筆記放入 <code>reports/_people/{display_name}/index.md</code>，詳見 README 遷移說明。</p>`
    }

    const observation = hasObservation
      ? `<section class="trends-section">
          <h3>長期觀察</h3>
          <div class="markdown-body">${renderMarkdownText(trends!.long_term_observation)}</div>
        </section>`
      : ''

    const timeline = hasTimeline
      ? `<section class="trends-section">
          <h3>成長軌跡</h3>
          ${trends!
            .growth_timeline.map(
              (entry) => `<article class="timeline-entry">
                <h4>${escapeHtml(entry.month)}</h4>
                <div class="markdown-body">${renderMarkdownText(entry.content)}</div>
              </article>`,
            )
            .join('')}
        </section>`
      : ''

    const pending = hasPending
      ? `<section class="trends-section">
          <h3>歷史待確認</h3>
          <ul class="historical-pending-list">${trends!
            .historical_pending.map((entry) => {
              const monthLabel =
                entry.status === 'resolved' && entry.resolved_month
                  ? `[${entry.raised_month}→${entry.resolved_month}]`
                  : `[${entry.raised_month}]`
              return `<li class="historical-pending-item ${entry.status}">
                  <span class="historical-pending-icon" aria-hidden="true">${entry.status === 'resolved' ? '✓' : '⚠'}</span>
                  <span class="historical-pending-date">${escapeHtml(monthLabel)}</span>
                  <span>${escapeHtml(entry.question)}</span>
                  ${entry.resolution_note ? `<span class="historical-pending-note"> — ${escapeHtml(entry.resolution_note)}</span>` : ''}
                </li>`
            })
            .join('')}</ul>
        </section>`
      : ''

    return `<div class="trends-panel">${observation}${timeline}${pending}</div>`
  }

  private renderProjectCard(project: LatestReportItem): string {
    const stats = [
      project.mr_count != null ? `MR ${project.mr_count}` : null,
      project.commit_count != null ? `Commits ${project.commit_count}` : null,
    ]
      .filter(Boolean)
      .join(' · ')

    return `<article class="project-card ${project.is_read ? 'read' : 'unread'}">
      <header>
        <h3>${escapeHtml(project.project_name)}</h3>
        ${project.is_read ? '' : '<span class="badge">未讀</span>'}
      </header>
      ${project.one_line ? `<p class="one-line">${escapeHtml(project.one_line)}</p>` : ''}
      ${stats ? `<p class="stats">${escapeHtml(stats)}</p>` : ''}
      ${renderSection('本週重點', project.highlights)}
      ${renderSection('成長面向', project.growth)}
      ${this.renderPendingItemsSection(project.pending_items)}
    </article>`
  }

  private renderPendingItemsSection(pendingItems: PendingItem[]): string {
    if (pendingItems.length === 0) {
      return ''
    }
    const rows = pendingItems
      .map((item) => {
        const disabled = this.resolvingPendingItemIds.has(item.id)
        return `<li class="pending-item-row">
          <label class="pending-item-checkbox">
            <input type="checkbox" data-pending-item-id="${item.id}" ${disabled ? 'disabled' : ''} />
            <span>${escapeHtml(item.question)}</span>
          </label>
        </li>`
      })
      .join('')
    return `<section class="section">
      <h4>待確認</h4>
      <ul class="pending-item-list">${rows}</ul>
    </section>`
  }

  private removePendingItemFromReports(itemId: number): void {
    if (!this.latestReports) {
      return
    }
    this.latestReports = {
      ...this.latestReports,
      projects: this.latestReports.projects.map((project) => ({
        ...project,
        pending_items: project.pending_items.filter((item) => item.id !== itemId),
      })),
    }
  }

  private async refreshAfterPendingItemResolved(): Promise<void> {
    await Promise.all([this.loadPeople(), this.loadDashboard(), this.loadPersonTrends()])
  }

  private async handleResolvePendingItem(itemId: number): Promise<void> {
    if (this.resolvingPendingItemIds.has(itemId)) {
      return
    }
    this.resolvingPendingItemIds.add(itemId)
    await this.renderWithStatus()
    try {
      await resolvePendingItem(itemId)
      this.removePendingItemFromReports(itemId)
      await this.refreshAfterPendingItemResolved()
      this.bannerMessage = '已標記為已釐清'
      this.bannerIsError = false
    } catch (error) {
      if (error instanceof ApiError && error.status === 502) {
        this.removePendingItemFromReports(itemId)
        await this.refreshAfterPendingItemResolved()
        this.bannerMessage = '已標記為已釐清，但歷史檔案同步失敗；趨勢頁可能尚未更新'
        this.bannerIsError = true
      } else {
        this.bannerMessage = error instanceof Error ? error.message : '閉環失敗'
        this.bannerIsError = true
      }
    } finally {
      this.resolvingPendingItemIds.delete(itemId)
      await this.renderWithStatus()
    }
  }

  private async switchAppView(view: AppView): Promise<void> {
    this.appView = view
    if (view === 'dashboard' && this.dashboard === null) {
      await this.loadDashboard()
    }
    if (view === 'mr-inbox') {
      await this.loadMrReviews()
    }
    if (view === 'projects') {
      if (this.projects.length === 0) {
        await this.loadProjects()
      }
      if (!this.projectDraft) {
        this.syncProjectDraft()
      }
      if (this.activeRunId !== null && this.activeRun === null) {
        try {
          this.activeRun = await fetchRun(this.activeRunId)
        } catch {
          // poll loop will recover
        }
      }
    }
    if (view === 'people') {
      await this.loadPeople()
      if (this.isCreatingPerson) {
        this.personDetail = null
      } else if (this.selectedSettingsPersonId !== null) {
        await this.loadPersonDetail(this.selectedSettingsPersonId)
      } else if (this.people.length > 0) {
        this.selectedSettingsPersonId = this.people[0].id
        await this.loadPersonDetail(this.selectedSettingsPersonId)
      }
    }
    this.render(await this.statusLine())
  }

  private async openPersonReport(personId: number): Promise<void> {
    this.appView = 'reports'
    this.selectedPersonId = personId
    this.personViewTab = 'weekly'
    await Promise.all([this.loadLatestReports(), this.loadPersonTrends()])
    this.render(await this.statusLine())
  }

  private mrDraftBadge(): string {
    const count = this.dashboard?.stats.mr_draft_count ?? 0
    if (count <= 0) {
      return ''
    }
    return `<span class="nav-badge">${count}</span>`
  }

  private async loadMrReviews(): Promise<void> {
    try {
      this.mrReviews = await fetchMrReviews(this.mrStatusFilter)
      if (
        this.selectedMrReviewId !== null &&
        !this.mrReviews.some((item) => item.id === this.selectedMrReviewId)
      ) {
        this.selectedMrReviewId = this.mrReviews[0]?.id ?? null
        this.syncMrEditorFromSelection()
        this.mrChatMessages = []
      } else if (this.selectedMrReviewId === null && this.mrReviews.length > 0) {
        this.selectMrReview(this.mrReviews[0].id)
      } else if (this.selectedMrReviewId !== null && !this.mrEditorDirty) {
        this.syncMrEditorFromSelection()
      }
    } catch (error) {
      this.mrReviews = []
      this.bannerMessage = error instanceof Error ? error.message : '無法載入 MR 草稿'
      this.bannerIsError = true
    }
  }

  private async switchMrStatusFilter(status: MrReviewStatus): Promise<void> {
    if (this.mrStatusFilter === status) {
      return
    }
    this.mrStatusFilter = status
    this.selectedMrReviewId = null
    this.mrEditorBody = ''
    this.mrEditorDirty = false
    this.mrChatMessages = []
    await this.loadMrReviews()
    this.render(await this.statusLine())
  }

  private selectMrReview(id: number): void {
    this.selectedMrReviewId = id
    this.mrEditorDirty = false
    this.mrChatMessages = []
    this.syncMrEditorFromSelection()
  }

  private get selectedMrReview(): MrReviewItem | null {
    if (this.selectedMrReviewId === null) {
      return null
    }
    return this.mrReviews.find((item) => item.id === this.selectedMrReviewId) ?? null
  }

  private syncMrEditorFromSelection(): void {
    const selected = this.selectedMrReview
    this.mrEditorBody = selected?.draft_body ?? ''
  }

  private renderMrInbox(): string {
    const filters: Array<{ key: MrReviewStatus; label: string }> = [
      { key: 'draft', label: '待發佈' },
      { key: 'published', label: '已發佈' },
      { key: 'ignored', label: '已忽略' },
    ]

    const filterTabs = filters
      .map(
        (filter) =>
          `<button type="button" class="mr-filter-tab ${this.mrStatusFilter === filter.key ? 'active' : ''}" data-mr-status="${filter.key}">${filter.label}</button>`,
      )
      .join('')

    const listItems =
      this.mrReviews.length === 0
        ? `<p class="mr-list-empty">尚無${this.mrStatusLabel(this.mrStatusFilter)}的 MR review</p>`
        : this.mrReviews
            .map((item) => {
              const active = item.id === this.selectedMrReviewId ? ' active' : ''
              const author = item.author_name ?? '未歸戶'
              const title = item.mr_title ?? `MR !${item.mr_iid}`
              return `<button type="button" class="mr-list-item${active}" data-mr-review-id="${item.id}">
                <span class="mr-list-title">!${item.mr_iid} ${escapeHtml(title)}</span>
                <span class="mr-list-meta">${escapeHtml(item.project_name)} · ${escapeHtml(author)} · 第 ${item.review_round} 輪</span>
                <span class="mr-list-date">${escapeHtml(formatTimestamp(item.created_at))}</span>
              </button>`
            })
            .join('')

    const selected = this.selectedMrReview
    const detail = selected
      ? this.renderMrReviewDetail(selected)
      : `<div class="mr-detail-empty"><p>選擇左側草稿以檢視內容</p></div>`

    return `<div class="mr-inbox">
      <div class="mr-inbox-header">
        <div>
          <h2 class="mr-inbox-title">MR 收件匣</h2>
          <p class="mr-inbox-subtitle">AI 產出的 MR review 草稿，發佈前可編輯與追問</p>
        </div>
      </div>
      <div class="mr-inbox-body">
        <aside class="mr-inbox-list">
          <div class="mr-filter-tabs">${filterTabs}</div>
          <div class="mr-list">${listItems}</div>
        </aside>
        <section class="mr-inbox-detail">${detail}</section>
      </div>
    </div>`
  }

  private mrStatusLabel(status: MrReviewStatus): string {
    switch (status) {
      case 'draft':
        return '待發佈'
      case 'published':
        return '已發佈'
      case 'ignored':
        return '已忽略'
    }
  }

  private renderMrReviewDetail(item: MrReviewItem): string {
    const isDraft = item.status === 'draft'
    const readOnly = !isDraft
    const title = item.mr_title ?? `MR !${item.mr_iid}`
    const author = item.author_name ?? '未歸戶'
    const sessionHint = isDraft
      ? item.agent_session_id
        ? `<span class="mr-session-badge">可追問 · ${escapeHtml(item.reviewer_agent)}</span>`
        : `<span class="mr-session-badge muted">無 agent session</span>`
      : ''

    const actions = isDraft
      ? `<div class="mr-detail-actions">
          <button id="mr-ignore" class="mr-btn-secondary" type="button" ${this.mrActionLoading ? 'disabled' : ''}>忽略</button>
          <button id="mr-save" class="mr-btn-secondary" type="button" ${this.mrActionLoading || !this.mrEditorDirty ? 'disabled' : ''}>儲存</button>
          <button id="mr-publish" class="mr-btn-primary" type="button" ${this.mrActionLoading ? 'disabled' : ''}>發佈到 GitLab</button>
        </div>`
      : ''

    const chatSection =
      isDraft && item.agent_session_id
        ? `<section class="mr-chat">
            <h3 class="mr-chat-title">追問 AI</h3>
            <div class="mr-chat-messages">
              ${
                this.mrChatMessages.length === 0
                  ? '<p class="mr-chat-empty">針對這份 review 向 AI 追問細節（不會自動發佈）</p>'
                  : this.mrChatMessages
                      .map(
                        (message) =>
                          `<div class="mr-chat-bubble ${message.role}">
                            <div class="mr-chat-role">${message.role === 'user' ? '你' : 'AI'}</div>
                            <div class="mr-chat-text">${escapeHtml(message.text)}</div>
                          </div>`,
                      )
                      .join('')
              }
              ${this.mrChatLoading ? '<p class="mr-chat-loading">AI 回覆中…</p>' : ''}
            </div>
            <div class="mr-chat-input-row">
              <textarea id="mr-chat-input" class="mr-chat-input" rows="2" placeholder="例如：為什麼你標記了 transaction helper？" ${this.mrChatLoading ? 'disabled' : ''}></textarea>
              <button id="mr-chat-send" class="mr-btn-primary" type="button" ${this.mrChatLoading ? 'disabled' : ''}>送出</button>
            </div>
          </section>`
        : ''

    return `<div class="mr-detail">
      <header class="mr-detail-header">
        <div>
          <h3 class="mr-detail-title">!${item.mr_iid} ${escapeHtml(title)}</h3>
          <p class="mr-detail-meta">${escapeHtml(item.project_name)} · ${escapeHtml(author)} · 第 ${item.review_round} 輪 · ${escapeHtml(this.mrStatusLabel(item.status))}</p>
        </div>
        ${sessionHint}
      </header>
      <label class="mr-editor-label" for="mr-editor">${readOnly ? 'Review 內容（唯讀）' : 'Review 草稿'}</label>
      <textarea id="mr-editor" class="mr-editor" ${readOnly ? 'readonly' : ''}>${escapeHtml(this.mrEditorBody)}</textarea>
      ${actions}
      ${chatSection}
    </div>`
  }

  private async handleMrSave(): Promise<void> {
    const selected = this.selectedMrReview
    if (!selected || selected.status !== 'draft' || this.mrActionLoading) {
      return
    }
    this.mrActionLoading = true
    await this.renderWithStatus()
    try {
      await updateMrReview(selected.id, this.mrEditorBody)
      this.mrEditorDirty = false
      await this.loadMrReviews()
      this.bannerMessage = '草稿已儲存'
      this.bannerIsError = false
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '儲存失敗'
      this.bannerIsError = true
    } finally {
      this.mrActionLoading = false
      await this.renderWithStatus()
    }
  }

  private async handleMrPublish(): Promise<void> {
    const selected = this.selectedMrReview
    if (!selected || selected.status !== 'draft' || this.mrActionLoading) {
      return
    }
    if (!window.confirm(`確定要將 MR !${selected.mr_iid} 的 review 發佈到 GitLab？`)) {
      return
    }
    if (this.mrEditorDirty) {
      try {
        await updateMrReview(selected.id, this.mrEditorBody)
        this.mrEditorDirty = false
      } catch (error) {
        this.bannerMessage = error instanceof Error ? error.message : '發佈前儲存失敗'
        this.bannerIsError = true
        await this.renderWithStatus()
        return
      }
    }
    this.mrActionLoading = true
    await this.renderWithStatus()
    try {
      await publishMrReview(selected.id)
      this.bannerMessage = `MR !${selected.mr_iid} 已發佈到 GitLab`
      this.bannerIsError = false
      this.selectedMrReviewId = null
      this.mrChatMessages = []
      await Promise.all([this.loadMrReviews(), this.loadDashboard()])
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '發佈失敗'
      this.bannerIsError = true
    } finally {
      this.mrActionLoading = false
      await this.renderWithStatus()
    }
  }

  private async handleMrIgnore(): Promise<void> {
    const selected = this.selectedMrReview
    if (!selected || selected.status !== 'draft' || this.mrActionLoading) {
      return
    }
    if (!window.confirm(`確定要忽略 MR !${selected.mr_iid} 的草稿？`)) {
      return
    }
    this.mrActionLoading = true
    await this.renderWithStatus()
    try {
      await ignoreMrReview(selected.id)
      this.bannerMessage = `已忽略 MR !${selected.mr_iid}`
      this.bannerIsError = false
      this.selectedMrReviewId = null
      this.mrChatMessages = []
      await Promise.all([this.loadMrReviews(), this.loadDashboard()])
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '忽略失敗'
      this.bannerIsError = true
    } finally {
      this.mrActionLoading = false
      await this.renderWithStatus()
    }
  }

  private async handleMrAgentTurn(): Promise<void> {
    const selected = this.selectedMrReview
    if (!selected || selected.status !== 'draft' || !selected.agent_session_id || this.mrChatLoading) {
      return
    }
    const input = this.root.querySelector('#mr-chat-input') as HTMLTextAreaElement | null
    const message = input?.value.trim()
    if (!message) {
      return
    }
    this.mrChatMessages.push({ role: 'user', text: message })
    if (input) {
      input.value = ''
    }
    this.mrChatLoading = true
    await this.renderWithStatus()
    try {
      const response = await agentTurnMrReview(selected.id, message)
      this.mrChatMessages.push({ role: 'assistant', text: response.reply })
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '追問失敗'
      this.bannerIsError = true
    } finally {
      this.mrChatLoading = false
      await this.renderWithStatus()
    }
  }

  private async handleMrScan(projectId: number, force = false): Promise<void> {
    if (this.activeRunId || this.reloading) {
      return
    }
    try {
      const response = await startMrScan(projectId, force ? { force: true } : undefined)
      this.activeRunId = response.run_id
      this.bannerMessage = null
      this.bannerIsError = false
      this.render(force ? `MR 強制重掃中 · run #${response.run_id}` : `MR 掃描中 · run #${response.run_id}`)
      this.startPolling(response.run_id)
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '無法啟動 MR 掃描'
      this.bannerIsError = true
      await this.renderWithStatus()
    }
  }

  private async selectPerson(personId: number): Promise<void> {
    this.selectedPersonId = personId
    await Promise.all([this.loadLatestReports(), this.loadPersonTrends()])
    this.render(await this.statusLine())
  }

  private async switchViewTab(tab: 'weekly' | 'trends'): Promise<void> {
    this.personViewTab = tab
    if (tab === 'trends' && this.personTrends === null) {
      await this.loadPersonTrends()
    }
    this.render(await this.statusLine())
  }

  private async handleMarkRead(): Promise<void> {
    if (!this.latestReports) {
      return
    }

    const unread = this.latestReports.projects.filter((item) => !item.is_read)
    for (const report of unread) {
      await markReportRead(report.id)
    }

    await this.loadPeople()
    await this.loadDashboard()
    this.render(await this.statusLine())
  }

  private async handleBindExisting(authorId: number, personId: number): Promise<void> {
    const author = this.unmatchedAuthors.find((item) => item.id === authorId)
    if (!author) {
      return
    }

    try {
      await bindIdentity(personId, author.kind, author.value)
      this.bannerMessage = `已將 ${author.value} 綁定到現有人員`
      await Promise.all([this.loadPeople(), this.loadUnmatchedAuthors()])
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '綁定失敗'
    }
    await this.renderWithStatus()
  }

  private async handleBindNew(authorId: number, displayName: string): Promise<void> {
    const author = this.unmatchedAuthors.find((item) => item.id === authorId)
    if (!author) {
      return
    }

    try {
      const person = await createPerson(displayName)
      await bindIdentity(person.id, author.kind, author.value)
      this.bannerMessage = `已建立 ${displayName} 並綁定 ${author.value}`
      await Promise.all([this.loadPeople(), this.loadUnmatchedAuthors()])
      this.selectedPersonId = person.id
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '建立或綁定失敗'
    }
    await this.renderWithStatus()
  }

  private async renderWithStatus(): Promise<void> {
    this.render(await this.statusLine())
  }

  private async handleReloadProjects(): Promise<void> {
    if (this.reloading || this.activeRunId) {
      return
    }

    this.reloading = true
    this.render('重新載入中…（clone / fetch 專案）')

    try {
      const result = await reloadProjects()
      const unhealthyNote = result.unhealthy > 0 ? ` · 異常 ${result.unhealthy}` : ''
      this.bannerMessage = `已重新佈建 ${result.total} 個專案（正常 ${result.healthy}${unhealthyNote}）`
      await Promise.all([
        this.loadPeople(),
        this.loadUnmatchedAuthors(),
        this.loadDashboard(),
        this.loadProjects(),
      ])
      this.syncProjectDraft()
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '重新載入失敗'
    } finally {
      this.reloading = false
      this.render(await this.statusLine())
    }
  }

  private async handleRunAll(): Promise<void> {
    try {
      const response = await startManualRun()
      this.activeRunId = response.run_id
      this.bannerMessage = null
      this.bannerIsError = false
      this.render(`執行中 · run #${response.run_id}`)
      this.startPolling(response.run_id)
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '無法啟動執行'
      this.bannerIsError = true
      this.render(await this.statusLine())
    }
  }

  private startPolling(runId: number): void {
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer)
    }
    if (this.runElapsedTimer !== null) {
      window.clearInterval(this.runElapsedTimer)
    }

    this.pollTimer = window.setInterval(() => {
      void this.pollRun(runId)
    }, 2000)
    this.runElapsedTimer = window.setInterval(() => {
      if (this.activeRunId === null) {
        return
      }
      this.runElapsedTick += 1
      if (this.appView === 'projects') {
        void this.renderWithStatus()
      }
      if (this.appView === 'mr-inbox') {
        void this.renderWithStatus()
      }
    }, 1000)
    void this.pollRun(runId)
  }

  private stopRunTimers(): void {
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer)
      this.pollTimer = null
    }
    if (this.runElapsedTimer !== null) {
      window.clearInterval(this.runElapsedTimer)
      this.runElapsedTimer = null
    }
    this.runElapsedTick = 0
  }

  private async pollRun(runId: number): Promise<void> {
    try {
      const run = await fetchRun(runId)
      this.activeRun = run
      if (!TERMINAL_STATUSES.has(run.status)) {
        if (this.appView === 'projects' || this.appView === 'mr-inbox') {
          await this.renderWithStatus()
        } else {
          this.render(`執行中 · run #${run.id} · ${run.status}`)
        }
        return
      }

      this.stopRunTimers()
      this.activeRunId = null
      this.activeRun = null
      const { message, isError } = formatRunCompleteBanner(run)
      this.bannerMessage = message
      this.bannerIsError = isError
      await Promise.all([
        this.loadPeople(),
        this.loadUnmatchedAuthors(),
        this.loadDashboard(),
        this.loadProjects(),
        this.appView === 'mr-inbox' ? this.loadMrReviews() : Promise.resolve(),
      ])
      if (this.appView === 'projects') {
        this.syncProjectDraft()
      }
      this.render(await this.statusLine())
    } catch (error) {
      this.stopRunTimers()
      this.activeRunId = null
      this.activeRun = null
      this.bannerMessage = error instanceof Error ? error.message : '輪詢失敗'
      this.bannerIsError = true
      this.render(await this.statusLine())
    }
  }

  private async statusLine(): Promise<string> {
    const health = await fetchHealth()
    return `已連線 · ${health.data_dir}`
  }

  private renderError(error: unknown): void {
    const message = error instanceof Error ? error.message : '無法連線後端'
    this.root.innerHTML = `
      <div class="layout error-state">
        <h1>Reviewer</h1>
        <p class="error">無法連線後端：${escapeHtml(message)}</p>
        <p class="hint">請確認 reviewer-server 已啟動。本地開發請保持 VITE_API_BASE 留空（走 Vite proxy）；跨域部署請設定 VITE_API_BASE 並在後端設定 CORS_ALLOW_ORIGINS。</p>
      </div>
    `
  }
}

function formatReportDateShort(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value)
  if (match) {
    return `${match[2]}-${match[3]}`
  }
  return value.length >= 5 ? value.slice(5) : value
}

function formatRunElapsed(startedAt: string | null, _tick: number): string {
  if (!startedAt) {
    return '00:00'
  }
  const started = Date.parse(startedAt.replace(' ', 'T'))
  if (Number.isNaN(started)) {
    return '00:00'
  }
  const elapsedSec = Math.max(0, Math.floor((Date.now() - started) / 1000))
  const minutes = Math.floor(elapsedSec / 60)
  const seconds = elapsedSec % 60
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

function formatTimestamp(value: string): string {
  return value.length >= 16 ? value.slice(0, 16) : value
}

function formatDurationSuffix(durationSec: number | null | undefined): string {
  if (durationSec == null || durationSec < 0) {
    return ''
  }
  const minutes = Math.floor(durationSec / 60)
  const seconds = durationSec % 60
  if (minutes > 0) {
    return ` · 耗時 ${minutes}m ${seconds}s`
  }
  return ` · 耗時 ${seconds}s`
}

function formatRunCompleteBanner(run: RunStatus): { message: string; isError: boolean } {
  const skipped =
    run.project_skipped > 0 ? `（略過 ${run.project_skipped} 個專案）` : ''
  const failed = run.projects.filter(
    (project) => project.state === 'failed' && project.error,
  )

  if (failed.length === 0) {
    return {
      message: `Run #${run.id} 已完成：${run.status}${skipped}`,
      isError: run.status === 'failed',
    }
  }

  const details = failed
    .map((project) => `${project.name}：${humanizeProjectError(project.error!)}`)
    .join('；')

  return {
    message: `Run #${run.id} 失敗${skipped} — ${details}`,
    isError: true,
  }
}

function humanizeProjectError(error: string): string {
  const lower = error.toLowerCase()
  if (
    lower.includes('認證失敗') ||
    lower.includes('authentication required') ||
    lower.includes('agent login')
  ) {
    return 'Cursor 登入已失效，請在本機重新執行 cursor-agent login 後再試'
  }
  return error
}

function renderMarkdownText(value: string): string {
  return escapeHtml(value).replaceAll('\n', '<br>')
}

function renderSection(title: string, items: string[]): string {
  if (items.length === 0) {
    return ''
  }
  return `<section class="section">
    <h4>${escapeHtml(title)}</h4>
    <ul>${items.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul>
  </section>`
}

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}
