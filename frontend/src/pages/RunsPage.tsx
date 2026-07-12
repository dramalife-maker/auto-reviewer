import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'

import { fetchRun, fetchRuns } from '../api'
import { Card, ListRow, StatusPill } from '../components/ui'
import { useToast } from '../context/ToastContext.tsx'
import {
  formatDurationLabel,
  formatDurationSuffix,
  formatTimestamp,
  humanizeProjectError,
  humanizeTrigger,
  runStatusTone,
} from '../lib/format'
import type { RunListItem, RunStatus, SkipSummary } from '../types'

export function RunsPage() {
  const navigate = useNavigate()
  const params = useParams()
  const toast = useToast()
  const routeRunId = params.runId ? Number(params.runId) : null
  const [runs, setRuns] = useState<RunListItem[]>([])
  const [total, setTotal] = useState(0)
  const [selectedRunId, setSelectedRunId] = useState<number | null>(routeRunId)
  const [selectedRun, setSelectedRun] = useState<RunStatus | null>(null)
  const [loading, setLoading] = useState(true)

  const loadRuns = useCallback(async () => {
    setLoading(true)
    try {
      const response = await fetchRuns({ limit: 50 })
      setRuns(response.runs)
      setTotal(response.total)
      setSelectedRunId((current) => current ?? routeRunId ?? response.runs[0]?.id ?? null)
    } catch (error) {
      setRuns([])
      setTotal(0)
      toast.show(error instanceof Error ? error.message : '無法載入執行紀錄', true)
    } finally {
      setLoading(false)
    }
  }, [toast, routeRunId])

  useEffect(() => {
    void loadRuns()
  }, [loadRuns])

  useEffect(() => {
    if (routeRunId !== null && Number.isFinite(routeRunId)) {
      setSelectedRunId(routeRunId)
    }
  }, [routeRunId])

  useEffect(() => {
    if (selectedRunId === null) {
      setSelectedRun(null)
      return
    }

    let cancelled = false
    async function loadDetail() {
      try {
        const run = await fetchRun(selectedRunId!)
        if (!cancelled) {
          setSelectedRun(run)
        }
      } catch (error) {
        if (!cancelled) {
          setSelectedRun(null)
          toast.show(error instanceof Error ? error.message : `無法載入 Run #${selectedRunId}`, true)
        }
      }
    }
    void loadDetail()
    return () => {
      cancelled = true
    }
  }, [toast, selectedRunId])

  const subtitle = useMemo(
    () => `顯示最近 ${runs.length}／共 ${total} 筆 · 依開始時間新到舊`,
    [runs.length, total],
  )

  function selectRun(runId: number) {
    setSelectedRunId(runId)
    navigate(`/runs/${runId}`)
  }

  return (
    <div className="mx-auto flex max-w-[1280px] flex-col gap-5">
      <header>
        <h1 className="text-[28px] font-bold tracking-tight text-ink">執行紀錄</h1>
        <p className="mt-1 text-[13.5px] text-ink-muted">{subtitle}</p>
      </header>

      <div className="grid h-[620px] grid-cols-[300px_1fr] overflow-hidden rounded-xl border border-border bg-surface">
        <aside className="min-h-0 overflow-y-auto border-r border-border bg-page/60">
          {runs.length > 0 ? (
            runs.map((run) => (
              <ListRow
                key={run.id}
                active={run.id === selectedRunId}
                onClick={() => selectRun(run.id)}
                className="border-b border-border-subtle"
              >
                <span className="block">
                  <span className="flex items-center gap-2">
                    <StatusPill tone={runStatusTone(run.status)}>{run.status}</StatusPill>
                    <span className="font-semibold text-ink">
                      #{run.id} {humanizeTrigger(run.trigger)}
                    </span>
                  </span>
                  <span className="mt-1 block text-xs text-ink-muted">
                    {formatTimestamp(run.started_at)}
                    {formatDurationSuffix(run.duration_sec)}
                  </span>
                  <span className="mt-1 block text-xs text-ink-muted">
                    專案 {run.project_total ?? 0}
                    {run.project_skipped > 0 ? ` · 略過 ${run.project_skipped}` : ''}
                  </span>
                </span>
              </ListRow>
            ))
          ) : (
            <p className="p-5 text-sm text-ink-muted">{loading ? '載入中...' : '尚無執行紀錄'}</p>
          )}
        </aside>

        <main className="min-h-0 overflow-y-auto bg-surface p-5">
          {selectedRun ? <RunDetail run={selectedRun} /> : <EmptyDetail />}
        </main>
      </div>
    </div>
  )
}

function EmptyDetail() {
  return (
    <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-border text-sm text-ink-muted">
      選擇左側執行以檢視明細
    </div>
  )
}

function RunDetail({ run }: { run: RunStatus }) {
  const highlight = run.status === 'failed' || run.status === 'partial'
  return (
    <div className="flex flex-col gap-5">
      <Card className={['p-5', highlight ? 'border-danger-border bg-danger-tint/40' : ''].join(' ')}>
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-xl font-semibold text-ink">Run #{run.id}</h2>
          <StatusPill tone={runStatusTone(run.status)}>{run.status}</StatusPill>
        </div>
        <dl className="mt-4 grid grid-cols-[repeat(auto-fit,minmax(150px,1fr))] gap-3">
          <MetaItem label="觸發" value={humanizeTrigger(run.trigger)} />
          <MetaItem label="開始" value={formatTimestamp(run.started_at)} />
          <MetaItem label="結束" value={run.finished_at ? formatTimestamp(run.finished_at) : '-'} />
          <MetaItem label="耗時" value={formatDurationLabel(run.duration_sec)} />
          <MetaItem
            label="專案"
            value={`${run.project_total ?? 0}${run.project_skipped > 0 ? `（略過 ${run.project_skipped}）` : ''}`}
          />
          {run.note ? <MetaItem label="備註" value={run.note} /> : null}
        </dl>
      </Card>

      <section>
        <h3 className="mb-3 text-[15px] font-semibold text-ink">專案結果</h3>
        <div className="grid gap-3">
          {run.projects.length > 0 ? (
            run.projects.map((project) => (
              <Card
                key={project.name}
                className={[
                  'p-4',
                  project.state === 'failed' || project.state === 'skipped_timeout'
                    ? 'border-danger-border bg-danger-tint/40'
                    : '',
                ].join(' ')}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <strong className="text-ink">{project.name}</strong>
                    <StatusPill tone={runStatusTone(project.state)}>{project.state}</StatusPill>
                  </div>
                  <span className="text-xs text-ink-muted">{formatDurationLabel(project.duration_sec)}</span>
                </div>
                {project.error ? (
                  <p className="mt-3 rounded-md border border-danger-border bg-danger-tint p-3 text-[13px] text-danger">
                    {humanizeProjectError(project.error)}
                  </p>
                ) : null}
                {hasSkipSummary(project.skip_summary) ? (
                  <SkipSummaryCard summary={project.skip_summary!} />
                ) : null}
              </Card>
            ))
          ) : (
            <p className="rounded-lg border border-dashed border-border p-6 text-center text-sm text-ink-muted">
              此 run 沒有專案列
            </p>
          )}
        </div>
      </section>
    </div>
  )
}

function MetaItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border-subtle bg-page p-3">
      <dt className="text-[11px] font-semibold text-ink-faint">{label}</dt>
      <dd className="mt-1 text-[13px] text-ink">{value}</dd>
    </div>
  )
}

function hasSkipSummary(summary: SkipSummary | null | undefined): boolean {
  if (!summary) return false
  return Object.keys(summary.by_reason).length > 0 || summary.items.length > 0
}

function SkipSummaryCard({ summary }: { summary: SkipSummary }) {
  const grouped = new Map<string, number[]>()
  for (const item of summary.items) {
    const items = grouped.get(item.skip_reason) ?? []
    items.push(item.mr_iid)
    grouped.set(item.skip_reason, items)
  }

  return (
    <div className="mt-3 rounded-lg border border-danger-border bg-danger-tint p-3">
      <div className="text-[13px] font-semibold text-danger">MR Skip 摘要</div>
      <div className="mt-2 grid gap-2">
        {Object.entries(summary.by_reason).map(([reason, count]) => {
          const iids = grouped.get(reason) ?? []
          return (
            <div key={reason} className="rounded-md border border-danger-border bg-surface p-2">
              <div className="text-[12.5px] font-semibold text-danger">
                {reason} <span className="font-normal">×{count}</span>
              </div>
              <div className="mt-1 text-xs text-ink-muted">
                {iids.length > 0 ? iids.map((iid) => `!${iid}`).join(', ') : '（items 已截斷）'}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
