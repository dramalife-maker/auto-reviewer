import { useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router-dom'

import {
  fetchLatestReports,
  fetchPeople,
  fetchPersonTrends,
  markReportRead,
  resolvePendingItem,
} from '../api'
import { Avatar } from '../components/ui/Avatar.tsx'
import { Button } from '../components/ui/Button.tsx'
import { Card } from '../components/ui/Card.tsx'
import { StatusPill } from '../components/ui/StatusPill.tsx'
import { Tabs } from '../components/ui/Tabs.tsx'
import { useBanner } from '../context/BannerContext.tsx'
import type {
  LatestReportItem,
  LatestReportsResponse,
  PendingItem,
  Person,
  PersonTrendsResponse,
} from '../types'

type ReportTab = 'overview' | 'trends' | `project:${string}`

export function ReportsPage() {
  const { personId } = useParams<{ personId?: string }>()
  const numericPersonId = parsePersonId(personId)
  const { show } = useBanner()
  const [people, setPeople] = useState<Person[]>([])
  const [reports, setReports] = useState<LatestReportsResponse | null>(null)
  const [trends, setTrends] = useState<PersonTrendsResponse | null>(null)
  const [trendsRequestedFor, setTrendsRequestedFor] = useState<number | null>(null)
  const [activeTab, setActiveTab] = useState<ReportTab>('overview')
  const [loading, setLoading] = useState(false)
  const [trendsLoading, setTrendsLoading] = useState(false)
  const [markingRead, setMarkingRead] = useState(false)
  const [resolvingIds, setResolvingIds] = useState<Set<number>>(new Set())

  useEffect(() => {
    setActiveTab('overview')
    setTrends(null)
    setTrendsRequestedFor(null)
  }, [numericPersonId])

  useEffect(() => {
    let cancelled = false

    async function loadPeople() {
      try {
        const nextPeople = await fetchPeople()
        if (!cancelled) {
          setPeople(nextPeople)
        }
      } catch (error) {
        if (!cancelled) {
          show(error instanceof Error ? error.message : '無法載入人員', true)
        }
      }
    }

    void loadPeople()
    return () => {
      cancelled = true
    }
  }, [show])

  useEffect(() => {
    if (numericPersonId === null) {
      setReports(null)
      return
    }

    let cancelled = false
    setLoading(true)
    fetchLatestReports(numericPersonId)
      .then((nextReports) => {
        if (!cancelled) setReports(nextReports)
      })
      .catch((error) => {
        if (!cancelled) {
          setReports(null)
          show(error instanceof Error ? error.message : '無法載入週報', true)
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [numericPersonId, show])

  useEffect(() => {
    if (
      numericPersonId === null ||
      activeTab !== 'trends' ||
      trendsRequestedFor === numericPersonId
    ) {
      return
    }

    let cancelled = false
    setTrendsRequestedFor(numericPersonId)
    setTrendsLoading(true)
    fetchPersonTrends(numericPersonId)
      .then((nextTrends) => {
        if (!cancelled) setTrends(nextTrends)
      })
      .catch((error) => {
        if (!cancelled) {
          show(error instanceof Error ? error.message : '無法載入成長趨勢', true)
        }
      })
      .finally(() => {
        if (!cancelled) setTrendsLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [activeTab, numericPersonId, show, trendsRequestedFor])

  const person = people.find((item) => item.id === numericPersonId) ?? null
  const reportProjects = reports?.projects ?? []
  const unreadReports = reportProjects.filter((project) => !project.is_read)
  const allPending = reportProjects.flatMap((project) =>
    project.pending_items.map((item) => ({ projectName: project.project_name, item })),
  )
  const openPending = allPending.filter(({ item }) => item.status === 'open')
  const tabItems = useMemo(
    () => [
      { id: 'overview', label: '總覽' },
      ...reportProjects.map((project) => ({
        id: projectTabId(project.project_name),
        label: project.project_name,
      })),
      { id: 'trends', label: '成長趨勢' },
    ],
    [reportProjects],
  )

  async function handleMarkRead() {
    if (!reports || unreadReports.length === 0 || markingRead) {
      return
    }

    setMarkingRead(true)
    try {
      await Promise.all(unreadReports.map((report) => markReportRead(report.id)))
      setReports({
        ...reports,
        projects: reports.projects.map((project) => ({ ...project, is_read: true })),
      })
      show('已標記為已讀')
    } catch (error) {
      show(error instanceof Error ? error.message : '標記已讀失敗', true)
    } finally {
      setMarkingRead(false)
    }
  }

  async function handleResolvePending(item: PendingItem) {
    if (item.status === 'resolved' || resolvingIds.has(item.id)) {
      return
    }

    setResolvingIds((current) => new Set(current).add(item.id))
    try {
      await resolvePendingItem(item.id)
      setReports((current) => markPendingResolved(current, item.id))
      setTrends(null)
      setTrendsRequestedFor(null)
      show('已標記為已釐清')
    } catch (error) {
      show(error instanceof Error ? error.message : '閉環失敗', true)
    } finally {
      setResolvingIds((current) => {
        const next = new Set(current)
        next.delete(item.id)
        return next
      })
    }
  }

  if (numericPersonId === null) {
    return (
      <Card className="mx-auto max-w-[800px] p-6">
        <p className="text-sm text-ink-muted">請從左側 sidebar 選擇一位人員來閱讀週報。</p>
      </Card>
    )
  }

  return (
    <Card className="mx-auto max-w-[800px] p-5 sm:px-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <Avatar name={person?.display_name ?? trends?.display_name ?? '未知人員'} />
          <div className="min-w-0">
            <h2 className="truncate text-[17px] font-semibold">
              {person?.display_name ?? trends?.display_name ?? `Person #${numericPersonId}`}
            </h2>
            <p className="text-xs text-ink-meta">
              {reports?.report_date ? `報告日期：${reports.report_date}` : '尚無報告日期'}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button disabled variant="secondary" title="目前 API 尚未提供完整 md raw 端點">
            完整 md
          </Button>
          {unreadReports.length === 0 ? (
            <StatusPill tone="success" className="px-3 py-2 text-[13px]">
              已讀
            </StatusPill>
          ) : (
            <Button disabled={markingRead} onClick={handleMarkRead} variant="secondary">
              {markingRead ? '標記中...' : `標記已讀 (${unreadReports.length})`}
            </Button>
          )}
        </div>
      </header>

      <div className="mt-5">
        <Tabs items={tabItems} value={activeTab} onChange={(id) => setActiveTab(id as ReportTab)} />
      </div>

      <div className="mt-5">
        {loading ? (
          <p className="text-sm text-ink-muted">載入週報中...</p>
        ) : reportProjects.length === 0 && activeTab !== 'trends' ? (
          <p className="rounded-lg bg-page p-4 text-sm text-ink-muted">尚無週報</p>
        ) : activeTab === 'overview' ? (
          <OverviewTab
            projects={reportProjects}
            openPending={openPending}
            resolvingIds={resolvingIds}
            onResolve={handleResolvePending}
          />
        ) : activeTab === 'trends' ? (
          <TrendsTab loading={trendsLoading} trends={trends} />
        ) : (
          <ProjectTab
            project={reportProjects.find((project) => projectTabId(project.project_name) === activeTab)}
            resolvingIds={resolvingIds}
            onResolve={handleResolvePending}
          />
        )}
      </div>
    </Card>
  )
}

function OverviewTab({
  projects,
  openPending,
  resolvingIds,
  onResolve,
}: {
  projects: LatestReportItem[]
  openPending: Array<{ projectName: string; item: PendingItem }>
  resolvingIds: Set<number>
  onResolve: (item: PendingItem) => void
}) {
  return (
    <div className="space-y-5">
      <div className="rounded-lg bg-page p-4 text-sm leading-6 text-ink-secondary">
        本週涵蓋 {projects.length} 個專案：
        {projects.map((project) => project.one_line || project.project_name).join('；') || '尚無摘要'}。
      </div>

      <div className="grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] gap-3">
        {projects.map((project) => {
          const openCount = project.pending_items.filter((item) => item.status === 'open').length
          return (
            <div
              key={project.id}
              className={[
                'rounded-lg border p-3',
                openCount > 0
                  ? 'border-warning-border bg-warning-tint text-warning-ink'
                  : 'border-border-subtle bg-page text-ink-muted',
              ].join(' ')}
            >
              <div className="font-semibold text-ink">{project.project_name}</div>
              <div className="mt-1 text-xs">
                活躍 · {openCount > 0 ? `${openCount} 待確認` : '無待確認'}
              </div>
            </div>
          )
        })}
      </div>

      <MergedList title="本週重點" projects={projects} pickItems={(project) => project.highlights} />
      <MergedList title="成長面向" projects={projects} pickItems={(project) => project.growth} />

      {openPending.length > 0 ? (
        <section className="rounded-lg border border-warning-border bg-warning-tint p-4 text-warning-ink">
          <h3 className="font-semibold">待確認彙整（1on1 時詢問）</h3>
          <div className="mt-3 space-y-3">
            {groupPendingByProject(openPending).map(([projectName, items]) => (
              <div key={projectName}>
                <div className="text-sm font-semibold text-ink">{projectName}</div>
                <PendingCheckboxList
                  items={items}
                  resolvingIds={resolvingIds}
                  onResolve={onResolve}
                />
              </div>
            ))}
          </div>
        </section>
      ) : null}
    </div>
  )
}

function ProjectTab({
  project,
  resolvingIds,
  onResolve,
}: {
  project: LatestReportItem | undefined
  resolvingIds: Set<number>
  onResolve: (item: PendingItem) => void
}) {
  if (!project) {
    return <p className="text-sm text-ink-muted">找不到專案報告</p>
  }

  return (
    <div className="space-y-5">
      {project.one_line ? (
        <p className="rounded-lg bg-page p-4 text-sm text-ink-secondary">{project.one_line}</p>
      ) : null}
      <BulletSection title="亮點" items={project.highlights} />
      <BulletSection title="成長觀察" items={project.growth} />
      <section>
        <h3 className="text-sm font-semibold">待確認</h3>
        {project.pending_items.length === 0 ? (
          <p className="mt-2 text-sm text-ink-muted">無待確認項目</p>
        ) : (
          <PendingCheckboxList
            items={project.pending_items}
            resolvingIds={resolvingIds}
            onResolve={onResolve}
          />
        )}
      </section>
    </div>
  )
}

function TrendsTab({
  loading,
  trends,
}: {
  loading: boolean
  trends: PersonTrendsResponse | null
}) {
  if (loading) {
    return <p className="text-sm text-ink-muted">載入成長趨勢中...</p>
  }

  if (!trends) {
    return <p className="text-sm text-ink-muted">尚無長期觀察資料。</p>
  }

  return (
    <div className="space-y-5">
      <section>
        <h3 className="text-sm font-semibold">長期觀察</h3>
        <p className="mt-2 whitespace-pre-wrap rounded-lg bg-page p-4 text-sm leading-6 text-ink-secondary">
          {trends.long_term_observation || '尚無長期觀察'}
        </p>
      </section>
      <section>
        <h3 className="text-sm font-semibold">成長時間軸</h3>
        <div className="mt-2 divide-y divide-border-subtle rounded-lg border border-border-subtle">
          {trends.growth_timeline.length === 0 ? (
            <p className="p-4 text-sm text-ink-muted">尚無時間軸資料</p>
          ) : (
            trends.growth_timeline.map((entry) => (
              <article key={entry.month} className="p-4">
                <h4 className="text-sm font-semibold">{entry.month}</h4>
                <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-ink-secondary">
                  {entry.content}
                </p>
              </article>
            ))
          )}
        </div>
      </section>
    </div>
  )
}

function MergedList({
  title,
  projects,
  pickItems,
}: {
  title: string
  projects: LatestReportItem[]
  pickItems: (project: LatestReportItem) => string[]
}) {
  const rows = projects.flatMap((project) =>
    pickItems(project).map((item) => ({ projectName: project.project_name, item })),
  )
  if (rows.length === 0) {
    return null
  }
  return (
    <section>
      <h3 className="text-sm font-semibold">{title}</h3>
      <ul className="mt-2 list-disc space-y-1 pl-5 text-sm leading-6 text-ink-secondary">
        {rows.map((row, index) => (
          <li key={`${row.projectName}-${index}`}>
            <strong>{row.projectName}</strong>：{row.item}
          </li>
        ))}
      </ul>
    </section>
  )
}

function BulletSection({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) {
    return null
  }
  return (
    <section>
      <h3 className="text-sm font-semibold">{title}</h3>
      <ul className="mt-2 list-disc space-y-1 pl-5 text-sm leading-6 text-ink-secondary">
        {items.map((item, index) => (
          <li key={index}>{item}</li>
        ))}
      </ul>
    </section>
  )
}

function PendingCheckboxList({
  items,
  resolvingIds,
  onResolve,
}: {
  items: PendingItem[]
  resolvingIds: Set<number>
  onResolve: (item: PendingItem) => void
}) {
  return (
    <ul className="mt-2 space-y-2 text-sm">
      {items.map((item) => {
        const resolved = item.status === 'resolved'
        return (
          <li key={item.id}>
            <label className="flex items-start gap-2">
              <input
                checked={resolved}
                className="mt-1"
                disabled={resolved || resolvingIds.has(item.id)}
                onChange={(event) => {
                  if (event.target.checked) onResolve(item)
                }}
                type="checkbox"
              />
              <span className={resolved ? 'text-ink-muted line-through' : ''}>{item.question}</span>
            </label>
          </li>
        )
      })}
    </ul>
  )
}

function groupPendingByProject(
  pending: Array<{ projectName: string; item: PendingItem }>,
): Array<[string, PendingItem[]]> {
  const grouped = new Map<string, PendingItem[]>()
  for (const entry of pending) {
    grouped.set(entry.projectName, [...(grouped.get(entry.projectName) ?? []), entry.item])
  }
  return Array.from(grouped.entries())
}

function markPendingResolved(
  reports: LatestReportsResponse | null,
  itemId: number,
): LatestReportsResponse | null {
  if (!reports) {
    return reports
  }
  return {
    ...reports,
    projects: reports.projects.map((project) => ({
      ...project,
      pending_items: project.pending_items.map((item) =>
        item.id === itemId
          ? {
              ...item,
              status: 'resolved',
              resolved_date: new Date().toISOString().slice(0, 10),
            }
          : item,
      ),
    })),
  }
}

function parsePersonId(personId: string | undefined): number | null {
  if (!personId) {
    return null
  }
  const parsed = Number(personId)
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null
}

function projectTabId(projectName: string): `project:${string}` {
  return `project:${projectName}`
}
