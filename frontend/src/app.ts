import {
  bindIdentity,
  createPerson,
  fetchDashboard,
  fetchHealth,
  fetchLatestReports,
  fetchPeople,
  fetchPersonTrends,
  fetchRun,
  fetchUnmatchedAuthors,
  markReportRead,
  reloadProjects,
  startManualRun,
} from './api'
import type {
  DashboardResponse,
  LatestReportItem,
  LatestReportsResponse,
  Person,
  PersonTrendsResponse,
  RunStatus,
  UnmatchedAuthor,
} from './types'

const TERMINAL_STATUSES = new Set(['success', 'partial', 'failed'])
type AppView = 'dashboard' | 'reports'

export class ReviewerApp {
  private root: HTMLElement
  private people: Person[] = []
  private selectedPersonId: number | null = null
  private personViewTab: 'weekly' | 'trends' = 'weekly'
  private latestReports: LatestReportsResponse | null = null
  private personTrends: PersonTrendsResponse | null = null
  private activeRunId: number | null = null
  private pollTimer: number | null = null
  private bannerMessage: string | null = null
  private bannerIsError = false
  private reloading = false
  private unmatchedAuthors: UnmatchedAuthor[] = []
  private showUnmatchedPanel = false
  private appView: AppView = 'dashboard'
  private dashboard: DashboardResponse | null = null

  constructor(root: HTMLElement) {
    this.root = root
  }

  async init(): Promise<void> {
    try {
      const health = await fetchHealth()
      this.bannerMessage = null
      await Promise.all([this.loadPeople(), this.loadUnmatchedAuthors(), this.loadDashboard()])
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
            <h1>1on1 Reviewer</h1>
            <p class="status-line">${escapeHtml(statusLine)}</p>
          </div>
          <div class="header-actions">
            <button id="toggle-unmatched" class="secondary" type="button" ${this.unmatchedAuthors.length === 0 ? 'disabled' : ''}>
              未歸戶 (${this.unmatchedAuthors.length})
            </button>
            <button id="reload-projects" class="secondary" type="button" ${this.reloading || this.activeRunId ? 'disabled' : ''}>
              ${this.reloading ? '載入中…' : '重新載入'}
            </button>
          </div>
        </header>
        ${this.bannerMessage ? `<div class="banner${this.bannerIsError ? ' error' : ''}">${escapeHtml(this.bannerMessage)}</div>` : ''}
        ${this.showUnmatchedPanel ? this.renderUnmatchedPanel() : ''}
        <div class="main">
          <aside class="sidebar">
            <nav class="sidebar-nav" aria-label="主要導覽">
              <button type="button" class="nav-item ${this.appView === 'dashboard' ? 'active' : ''}" data-nav="dashboard">
                控制台
              </button>
              <button type="button" class="nav-item ${this.appView === 'reports' ? 'active' : ''}" data-nav="reports">
                報告閱讀器
              </button>
            </nav>
            ${this.appView === 'reports' ? `<h2>人員</h2>${this.renderPeopleList()}` : ''}
          </aside>
          <section class="content">
            ${this.appView === 'dashboard' ? this.renderDashboard() : this.renderContent()}
          </section>
        </div>
      </div>
    `

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

    this.root.querySelectorAll('[data-view-tab]').forEach((element) => {
      element.addEventListener('click', () => {
        const tab = (element as HTMLElement).dataset.viewTab as 'weekly' | 'trends'
        void this.switchViewTab(tab)
      })
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
      </div>

      <div class="dashboard-panels">
        <section class="dashboard-panel">
          <h3 class="panel-title">最近報告</h3>
          <div class="recent-list">${recentRows}</div>
        </section>
        <section class="dashboard-panel">
          <h3 class="panel-title">排程</h3>
          <div class="schedule-row"><span aria-hidden="true">📅</span> ${escapeHtml(schedule?.label ?? '—')}</div>
          <div class="schedule-row muted">
            <span aria-hidden="true">🕒</span>
            ${schedule?.next_run_at ? `下次 ${escapeHtml(schedule.next_run_at)}` : '無下次排程'}
          </div>
          ${scheduleStatus}
        </section>
      </div>
    </div>`
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
          <ul>${trends!.historical_pending.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul>
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
      ${renderSection('待確認', project.pending)}
    </article>`
  }

  private async switchAppView(view: AppView): Promise<void> {
    this.appView = view
    if (view === 'dashboard' && this.dashboard === null) {
      await this.loadDashboard()
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
      this.bannerMessage = `已重新載入 ${result.total} 個專案（正常 ${result.healthy}${unhealthyNote}）`
      await Promise.all([this.loadPeople(), this.loadUnmatchedAuthors(), this.loadDashboard()])
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

    this.pollTimer = window.setInterval(() => {
      void this.pollRun(runId)
    }, 2000)
    void this.pollRun(runId)
  }

  private async pollRun(runId: number): Promise<void> {
    try {
      const run = await fetchRun(runId)
      if (!TERMINAL_STATUSES.has(run.status)) {
        this.render(`執行中 · run #${run.id} · ${run.status}`)
        return
      }

      if (this.pollTimer !== null) {
        window.clearInterval(this.pollTimer)
        this.pollTimer = null
      }
      this.activeRunId = null
      const { message, isError } = formatRunCompleteBanner(run)
      this.bannerMessage = message
      this.bannerIsError = isError
      await Promise.all([this.loadPeople(), this.loadUnmatchedAuthors(), this.loadDashboard()])
      this.render(await this.statusLine())
    } catch (error) {
      if (this.pollTimer !== null) {
        window.clearInterval(this.pollTimer)
        this.pollTimer = null
      }
      this.activeRunId = null
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
        <h1>1on1 Reviewer</h1>
        <p class="error">無法連線後端：${escapeHtml(message)}</p>
        <p class="hint">請確認 reviewer-server 已啟動。本地開發請保持 VITE_API_BASE 留空（走 Vite proxy）；跨域部署請設定 VITE_API_BASE 並在後端設定 CORS_ALLOW_ORIGINS。</p>
      </div>
    `
  }
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
