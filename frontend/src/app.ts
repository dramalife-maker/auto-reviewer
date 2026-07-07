import {
  fetchHealth,
  fetchLatestReports,
  fetchPeople,
  fetchRun,
  markReportRead,
  startManualRun,
} from './api'
import type { LatestReportItem, LatestReportsResponse, Person } from './types'

const TERMINAL_STATUSES = new Set(['success', 'partial', 'failed'])

export class ReviewerApp {
  private root: HTMLElement
  private people: Person[] = []
  private selectedPersonId: number | null = null
  private latestReports: LatestReportsResponse | null = null
  private activeRunId: number | null = null
  private pollTimer: number | null = null
  private bannerMessage: string | null = null

  constructor(root: HTMLElement) {
    this.root = root
  }

  async init(): Promise<void> {
    try {
      const health = await fetchHealth()
      this.bannerMessage = null
      await this.loadPeople()
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

  private render(statusLine: string): void {
    this.root.innerHTML = `
      <div class="layout">
        <header class="header">
          <div>
            <h1>1on1 Reviewer</h1>
            <p class="status-line">${escapeHtml(statusLine)}</p>
          </div>
          <button id="run-all" class="primary" type="button" ${this.activeRunId ? 'disabled' : ''}>
            ${this.activeRunId ? '執行中…' : '全部執行'}
          </button>
        </header>
        ${this.bannerMessage ? `<div class="banner">${escapeHtml(this.bannerMessage)}</div>` : ''}
        <div class="main">
          <aside class="sidebar">
            <h2>人員</h2>
            ${this.renderPeopleList()}
          </aside>
          <section class="content">
            ${this.renderContent()}
          </section>
        </div>
      </div>
    `

    this.root.querySelector('#run-all')?.addEventListener('click', () => {
      void this.handleRunAll()
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

    if (!this.latestReports || this.latestReports.projects.length === 0) {
      return `<div class="content-header">
        <h2>${escapeHtml(person.display_name)}</h2>
        <p class="empty">尚無週報</p>
      </div>`
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
    <div class="project-grid">
      ${this.latestReports.projects.map((project) => this.renderProjectCard(project)).join('')}
    </div>`
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

  private async selectPerson(personId: number): Promise<void> {
    this.selectedPersonId = personId
    await this.loadLatestReports()
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
    this.render(await this.statusLine())
  }

  private async handleRunAll(): Promise<void> {
    try {
      const response = await startManualRun()
      this.activeRunId = response.run_id
      this.bannerMessage = null
      this.render(`執行中 · run #${response.run_id}`)
      this.startPolling(response.run_id)
    } catch (error) {
      this.bannerMessage = error instanceof Error ? error.message : '無法啟動執行'
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
      this.bannerMessage = `Run #${run.id} 已完成：${run.status}（略過 ${run.project_skipped} 個專案）`
      await this.loadPeople()
      this.render(await this.statusLine())
    } catch (error) {
      if (this.pollTimer !== null) {
        window.clearInterval(this.pollTimer)
        this.pollTimer = null
      }
      this.activeRunId = null
      this.bannerMessage = error instanceof Error ? error.message : '輪詢失敗'
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
