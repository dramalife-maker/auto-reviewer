import { useEffect, useMemo, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import { useParams } from 'react-router-dom'
import remarkGfm from 'remark-gfm'

import {
  agentTurnPersonReportChat,
  fetchLatestReports,
  fetchPeople,
  fetchPersonReportChat,
  fetchPersonTrends,
  markReportRead,
  resolvePendingItem,
} from '../api'
import { Avatar } from '../components/ui/Avatar.tsx'
import { Button } from '../components/ui/Button.tsx'
import { Card } from '../components/ui/Card.tsx'
import { StatusPill } from '../components/ui/StatusPill.tsx'
import { Tabs } from '../components/ui/Tabs.tsx'
import { useToast } from '../context/ToastContext.tsx'
import type {
  LatestReportItem,
  LatestReportsResponse,
  PendingItem,
  PendingObservation,
  PendingObservationStatus,
  Person,
  PersonReportChatMessage,
  PersonTrendsResponse,
} from '../types'

/** Resolve person-level month file links like `./2026-05.md` → `2026-05`. */
function parsePersonMonthHref(href: string | undefined): string | null {
  if (!href) return null
  const path = href.split(/[?#]/, 2)[0] ?? ''
  const match = path.match(/(?:^\.\/)?(\d{4}-\d{2})\.md$/i)
  return match?.[1] ?? null
}

function isRelativeMarkdownHref(href: string | undefined): boolean {
  if (!href) return false
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//') || href.startsWith('#')) {
    return false
  }
  return href.split(/[?#]/, 2)[0]?.toLowerCase().endsWith('.md') ?? false
}

function MarkdownPreview({
  content,
  empty = '尚無內容',
  className = '',
  onMonthLink,
}: {
  content: string
  empty?: string
  className?: string
  /** When set, `./YYYY-MM.md` links jump to that month instead of navigating away. */
  onMonthLink?: (month: string) => void
}) {
  const trimmed = content.trim()
  return (
    <div
      aria-label="Markdown 預覽"
      className={['md-preview text-[13.5px] leading-6 text-ink-secondary', className]
        .filter(Boolean)
        .join(' ')}
    >
      {trimmed ? (
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={{
            a: ({ href, children, ...props }) => {
              const month = parsePersonMonthHref(href)
              if (month && onMonthLink) {
                return (
                  <a
                    {...props}
                    href={`#${trendMonthElementId(month)}`}
                    onClick={(event) => {
                      event.preventDefault()
                      onMonthLink(month)
                    }}
                  >
                    {children}
                  </a>
                )
              }
              if (isRelativeMarkdownHref(href)) {
                return (
                  <a
                    {...props}
                    href={href}
                    onClick={(event) => {
                      event.preventDefault()
                    }}
                    title="此相對路徑尚未支援跳轉"
                  >
                    {children}
                  </a>
                )
              }
              return (
                <a {...props} href={href}>
                  {children}
                </a>
              )
            },
          }}
        >
          {content}
        </ReactMarkdown>
      ) : (
        <p className="text-ink-muted">{empty}</p>
      )}
    </div>
  )
}

function trendMonthElementId(month: string): string {
  return `trend-month-${month}`
}

type ReportTab = 'overview' | 'trends' | `project:${string}`

type ChatBubble = {
  role: 'user' | 'assistant'
  text: string
}

function hydrateChat(messages: PersonReportChatMessage[]): ChatBubble[] {
  return messages.map((message) => ({
    role: message.role,
    text: message.content,
  }))
}

export function ReportsPage() {
  const { personId } = useParams<{ personId?: string }>()
  const numericPersonId = parsePersonId(personId)
  const { show } = useToast()
  const [people, setPeople] = useState<Person[]>([])
  const [reports, setReports] = useState<LatestReportsResponse | null>(null)
  const [trends, setTrends] = useState<PersonTrendsResponse | null>(null)
  const [trendsRequestedFor, setTrendsRequestedFor] = useState<number | null>(null)
  const [activeTab, setActiveTab] = useState<ReportTab>('overview')
  const [loading, setLoading] = useState(false)
  const [trendsLoading, setTrendsLoading] = useState(false)
  const [markingRead, setMarkingRead] = useState(false)
  const [resolvingIds, setResolvingIds] = useState<Set<number>>(new Set())
  const [chatOpen, setChatOpen] = useState(true)
  const [chatInput, setChatInput] = useState('')
  const [chatMessages, setChatMessages] = useState<ChatBubble[]>([])
  const [chatLoading, setChatLoading] = useState(false)

  useEffect(() => {
    setActiveTab('overview')
    setTrends(null)
    setTrendsRequestedFor(null)
    setChatInput('')
    setChatMessages([])
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
    if (numericPersonId === null) {
      setChatMessages([])
      return
    }

    let cancelled = false
    fetchPersonReportChat(numericPersonId)
      .then((chat) => {
        if (!cancelled) setChatMessages(hydrateChat(chat.chat_messages))
      })
      .catch((error) => {
        if (!cancelled) {
          setChatMessages([])
          show(error instanceof Error ? error.message : '無法載入 Agent Chat', true)
        }
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
        if (!cancelled) {
          // Mark after settle so updating trendsRequestedFor does not re-run
          // this effect and cancel the in-flight request (stuck loading).
          setTrendsRequestedFor(numericPersonId)
          setTrendsLoading(false)
        }
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

  async function reloadLatestReports() {
    if (numericPersonId === null) {
      return
    }
    const nextReports = await fetchLatestReports(numericPersonId)
    setReports(nextReports)
    setTrends(null)
    setTrendsRequestedFor(null)
  }

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

  async function handleAgentTurn() {
    if (numericPersonId === null || chatLoading) {
      return
    }
    const message = chatInput.trim()
    if (!message) {
      return
    }

    setChatMessages((current) => [...current, { role: 'user', text: message }])
    setChatInput('')
    setChatLoading(true)
    try {
      const response = await agentTurnPersonReportChat(numericPersonId, message)
      setChatMessages((current) => [...current, { role: 'assistant', text: response.reply }])
      if (response.ingest_warnings && response.ingest_warnings.length > 0) {
        show(`報告已更新，但有 ${response.ingest_warnings.length} 則 ingest 警告`, true)
      }
      await reloadLatestReports()
    } catch (error) {
      setChatMessages((current) => current.slice(0, -1))
      setChatInput(message)
      show(error instanceof Error ? error.message : 'Agent Chat 失敗', true)
    } finally {
      setChatLoading(false)
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
    <div className="mx-auto flex max-w-[1200px] gap-4">
      <Card className="min-w-0 flex-1 p-5 sm:px-6">
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

      {chatOpen ? (
        <Card className="flex w-[360px] shrink-0 flex-col overflow-hidden p-5">
          <ReportChatPanel
            chatInput={chatInput}
            chatLoading={chatLoading}
            chatMessages={chatMessages}
            onAgentTurn={handleAgentTurn}
            onChatInputChange={setChatInput}
            onCollapse={() => setChatOpen(false)}
          />
        </Card>
      ) : (
        <Card className="flex w-12 shrink-0 flex-col items-center overflow-hidden py-3">
          <Button
            aria-label="展開 Agent Chat"
            className="px-2 py-1.5 text-xs"
            onClick={() => setChatOpen(true)}
            variant="ghost"
          >
            Chat
          </Button>
        </Card>
      )}
    </div>
  )
}

function ReportChatPanel({
  chatMessages,
  chatInput,
  chatLoading,
  onChatInputChange,
  onAgentTurn,
  onCollapse,
}: {
  chatMessages: ChatBubble[]
  chatInput: string
  chatLoading: boolean
  onChatInputChange: (value: string) => void
  onAgentTurn: () => void
  onCollapse: () => void
}) {
  return (
    <div className="flex h-[min(70vh,720px)] min-h-0 flex-col">
      <div className="flex shrink-0 items-center justify-between gap-2">
        <h4 className="text-sm font-semibold">Agent Chat</h4>
        <Button
          aria-label="收合 Agent Chat"
          className="p-1.5"
          onClick={onCollapse}
          title="收合 Agent Chat"
          variant="ghost"
        >
          <svg aria-hidden="true" className="size-4" fill="currentColor" viewBox="0 0 48 48">
            <path d="M32.6,22.6a1.9,1.9,0,0,0,0,2.8l5.9,6a2.1,2.1,0,0,0,2.7.2,1.9,1.9,0,0,0,.2-3L38.8,26H44a2,2,0,0,0,0-4H38.8l2.6-2.6a1.9,1.9,0,0,0-.2-3,2.1,2.1,0,0,0-2.7.2Z" />
            <path d="M15.4,25.4a1.9,1.9,0,0,0,0-2.8l-5.9-6a2.1,2.1,0,0,0-2.7-.2,1.9,1.9,0,0,0-.2,3L9.2,22H4a2,2,0,0,0,0,4H9.2L6.6,28.6a1.9,1.9,0,0,0,.2,3,2.1,2.1,0,0,0,2.7-.2Z" />
            <path d="M26,6V42a2,2,0,0,0,4,0V6a2,2,0,0,0-4,0Z" />
            <path d="M22,42V6a2,2,0,0,0-4,0V42a2,2,0,0,0,4,0Z" />
          </svg>
        </Button>
      </div>
      <div className="mt-3 min-h-0 flex-1 space-y-3 overflow-y-auto rounded-lg bg-surface">
        {chatMessages.length === 0 ? (
          <p className="rounded-lg bg-page p-3 text-sm text-ink-muted">
            討論並調整這位人員的週報／觀察檔。
          </p>
        ) : (
          chatMessages.map((message, index) => (
            <div
              key={index}
              className={['flex', message.role === 'user' ? 'justify-end' : 'justify-start'].join(
                ' ',
              )}
            >
              <div
                className={[
                  'min-w-0 max-w-[85%] break-words whitespace-pre-wrap rounded-xl px-3 py-2 text-sm leading-6',
                  message.role === 'user' ? 'bg-mr-soft text-mr-dark' : 'bg-page text-ink-secondary',
                ].join(' ')}
              >
                {message.text}
              </div>
            </div>
          ))
        )}
        {chatLoading ? <p className="text-sm text-ink-muted">AI 回覆中...</p> : null}
      </div>
      <div className="mt-3 flex shrink-0 gap-2">
        <textarea
          className="max-h-28 min-h-[44px] flex-1 resize-y overflow-y-auto rounded-lg border border-border bg-surface p-2 text-sm outline-none focus:border-mr"
          disabled={chatLoading}
          onChange={(event) => onChatInputChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault()
              onAgentTurn()
            }
          }}
          placeholder="例如：把 alpha 的 one_line 改得更精準"
          value={chatInput}
        />
        <Button
          disabled={chatLoading || chatInput.trim().length === 0}
          onClick={onAgentTurn}
          variant="mr"
        >
          送出
        </Button>
      </div>
    </div>
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
        {projects.some((project) => project.one_line || project.highlights.length > 0)
          ? `本週涵蓋 ${projects.length} 個專案：${projects.map((project) => project.one_line || project.project_name).join('；')}。`
          : `尚無週報摘要；目前有 ${projects.length} 個專案的待折入觀察或待確認。`}
      </div>

      <div className="grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] gap-3">
        {projects.map((project) => {
          const openCount = project.pending_items.filter((item) => item.status === 'open').length
          return (
            <div
              key={project.project_name}
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

      <PendingObservationsSection
        groups={groupObservationsByProject(
          projects.flatMap((project) =>
            (project.pending_observations ?? []).map((observation) => ({
              projectName: project.project_name,
              observation,
            })),
          ),
        )}
      />

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
      <PendingObservationsSection
        groups={
          (project.pending_observations ?? []).length === 0
            ? []
            : [[project.project_name, project.pending_observations ?? []]]
        }
      />
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

  const availableMonths = new Set(trends.growth_timeline.map((entry) => entry.month))

  function jumpToMonth(month: string) {
    if (!availableMonths.has(month)) {
      return
    }
    document.getElementById(trendMonthElementId(month))?.scrollIntoView({
      behavior: 'smooth',
      block: 'start',
    })
  }

  return (
    <div className="space-y-5">
      <section>
        <h3 className="text-sm font-semibold">長期觀察</h3>
        <MarkdownPreview
          className="mt-2 rounded-lg bg-page p-4"
          content={trends.long_term_observation}
          empty="尚無長期觀察"
          onMonthLink={jumpToMonth}
        />
      </section>
      <section>
        <h3 className="text-sm font-semibold">成長時間軸</h3>
        <div className="mt-2 divide-y divide-border-subtle rounded-lg border border-border-subtle">
          {trends.growth_timeline.length === 0 ? (
            <p className="p-4 text-sm text-ink-muted">尚無時間軸資料</p>
          ) : (
            trends.growth_timeline.map((entry) => (
              <article
                key={entry.month}
                id={trendMonthElementId(entry.month)}
                className="scroll-mt-4 p-4"
              >
                <h4 className="text-sm font-semibold">{entry.month}</h4>
                <MarkdownPreview
                  className="mt-2"
                  content={entry.content}
                  onMonthLink={jumpToMonth}
                />
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

function PendingObservationsSection({
  groups,
}: {
  groups: Array<[string, PendingObservation[]]>
}) {
  if (groups.length === 0) {
    return null
  }

  return (
    <section className="space-y-3">
      <h3 className="text-sm font-semibold">待折入觀察</h3>
      <div className="space-y-4">
        {groups.map(([projectName, observations]) => (
          <div key={projectName} className="space-y-3">
            {groups.length > 1 ? (
              <div className="text-sm font-semibold text-ink">{projectName}</div>
            ) : null}
            {observations.map((observation) => (
              <article
                key={`${projectName}-${observation.filename}`}
                className="rounded-lg border border-border-subtle bg-page p-4"
              >
                <header className="flex flex-wrap items-center gap-2">
                  <StatusPill tone={observationStatusTone(observation.status)}>
                    {observationStatusLabel(observation.status)}
                  </StatusPill>
                  <span className="text-sm font-semibold text-ink">
                    {observationIdentity(observation)}
                  </span>
                </header>
                <MarkdownPreview className="mt-3" content={observation.content} />
              </article>
            ))}
          </div>
        ))}
      </div>
    </section>
  )
}

function observationIdentity(observation: PendingObservation): string {
  if (observation.mr_title) {
    return `!${observation.mr_iid} ${observation.mr_title}（第 ${observation.review_round} 輪）`
  }
  return `MR !${observation.mr_iid}（第 ${observation.review_round} 輪）`
}

function observationStatusLabel(status: PendingObservationStatus): string {
  switch (status) {
    case 'published':
      return '已發佈・待折入'
    case 'draft':
      return '草稿'
    case 'ignored':
      return '已忽略'
    case 'unknown':
      return '未知'
  }
}

function observationStatusTone(
  status: PendingObservationStatus,
): 'success' | 'warning' | 'neutral' | 'mr' {
  switch (status) {
    case 'published':
      return 'success'
    case 'draft':
      return 'warning'
    case 'ignored':
      return 'neutral'
    case 'unknown':
      return 'mr'
  }
}

function groupObservationsByProject(
  rows: Array<{ projectName: string; observation: PendingObservation }>,
): Array<[string, PendingObservation[]]> {
  const grouped = new Map<string, PendingObservation[]>()
  for (const row of rows) {
    grouped.set(row.projectName, [...(grouped.get(row.projectName) ?? []), row.observation])
  }
  return Array.from(grouped.entries()).filter(([, observations]) => observations.length > 0)
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
