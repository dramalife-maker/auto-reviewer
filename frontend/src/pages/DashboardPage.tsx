import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'

import {
  catchUpSchedule,
  fetchDashboard,
  startManualRun,
  updateSchedule,
} from '../api'
import { Button, Card, Input, ListRow, StatCard, StatusPill } from '../components/ui'
import { useBanner } from '../context/BannerContext.tsx'
import { useRunPolling } from '../hooks/useRunPolling'
import { clearCatchUpDismiss, dismissCatchUp, isCatchUpDismissed } from '../lib/catchup'
import {
  formatDurationLabel,
  formatDurationSuffix,
  formatTimestamp,
  humanizeProjectError,
  humanizeTrigger,
  runStatusTone,
} from '../lib/format'
import type { DashboardResponse, RunStatus, ScheduleUpdateInput } from '../types'

const WEEKDAY_OPTIONS = [
  { value: 0, label: '週一' },
  { value: 1, label: '週二' },
  { value: 2, label: '週三' },
  { value: 3, label: '週四' },
  { value: 4, label: '週五' },
  { value: 5, label: '週六' },
  { value: 6, label: '週日' },
]

const emptyStats = {
  project_count: 0,
  person_count: 0,
  unread_count: 0,
  pending_count: 0,
  mr_draft_count: 0,
}

function formatRunCompleteBanner(run: RunStatus): { message: string; isError: boolean } {
  const skipped = run.project_skipped > 0 ? `（略過 ${run.project_skipped} 個專案）` : ''
  const failed = run.projects.filter((project) => project.state === 'failed' && project.error)

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
    message: `Run #${run.id} 失敗${skipped} - ${details}`,
    isError: true,
  }
}

export function DashboardPage() {
  const navigate = useNavigate()
  const banner = useBanner()
  const [dashboard, setDashboard] = useState<DashboardResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [scheduleSaving, setScheduleSaving] = useState(false)
  const [catchUpLoading, setCatchUpLoading] = useState(false)
  const [dismissVersion, setDismissVersion] = useState(0)
  const [scheduleForm, setScheduleForm] = useState<ScheduleUpdateInput>({
    enabled: true,
    weekday: 0,
    run_time: '09:00',
    tz_offset_min: 480,
    per_project_timeout_sec: 600,
    max_concurrency: 2,
    mr_poll_interval_min: 60,
  })

  const loadDashboard = useCallback(async () => {
    setLoading(true)
    try {
      setDashboard(await fetchDashboard())
    } catch (error) {
      banner.show(error instanceof Error ? error.message : '無法載入控制台', true)
    } finally {
      setLoading(false)
    }
  }, [banner])

  const { startPolling, activeRunId } = useRunPolling({
    onComplete: async (run) => {
      const result = formatRunCompleteBanner(run)
      banner.show(result.message, result.isError)
      await loadDashboard()
    },
  })

  useEffect(() => {
    void loadDashboard()
  }, [loadDashboard])

  useEffect(() => {
    const schedule = dashboard?.schedule
    if (!schedule) return
    setScheduleForm({
      enabled: schedule.enabled,
      weekday: schedule.weekday ?? 0,
      run_time: schedule.run_time,
      tz_offset_min: schedule.tz_offset_min,
      per_project_timeout_sec: schedule.per_project_timeout_sec,
      max_concurrency: schedule.max_concurrency,
      mr_poll_interval_min: schedule.mr_poll_interval_min,
    })
  }, [dashboard])

  const stats = dashboard?.stats ?? emptyStats
  const lastRunLine = dashboard?.last_run
    ? `上次執行 ${formatTimestamp(dashboard.last_run.started_at)}${formatDurationSuffix(
        dashboard.last_run.duration_sec,
      )}`
    : '尚無執行紀錄'
  const missed = dashboard?.schedule.missed_weekly_run ?? null
  const showCatchUp = useMemo(
    () => Boolean(missed && !isCatchUpDismissed(missed.due_at)),
    [missed, dismissVersion],
  )

  async function handleRunAll() {
    try {
      const response = await startManualRun()
      banner.dismiss()
      startPolling(response.run_id)
    } catch (error) {
      banner.show(error instanceof Error ? error.message : '無法啟動執行', true)
    }
  }

  async function handleCatchUp() {
    if (catchUpLoading || activeRunId !== null) return
    setCatchUpLoading(true)
    try {
      const response = await catchUpSchedule()
      clearCatchUpDismiss()
      banner.show(`已啟動補跑 · run #${response.run_id}`)
      await loadDashboard()
      startPolling(response.run_id)
    } catch (error) {
      banner.show(error instanceof Error ? error.message : '補跑失敗', true)
    } finally {
      setCatchUpLoading(false)
    }
  }

  function handleDismissCatchUp() {
    if (!missed) return
    dismissCatchUp(missed.due_at)
    setDismissVersion((value) => value + 1)
  }

  async function handleScheduleSave() {
    const runTime = String(scheduleForm.run_time ?? '').trim()
    const weekday = Number(scheduleForm.weekday)
    const tzOffset = Number(scheduleForm.tz_offset_min)
    const timeout = Number(scheduleForm.per_project_timeout_sec)
    const concurrency = Number(scheduleForm.max_concurrency)
    const mrPoll = Number(scheduleForm.mr_poll_interval_min)

    if (!/^\d{1,2}:\d{2}$/.test(runTime)) {
      banner.show('執行時間格式須為 HH:MM', true)
      return
    }
    if (!Number.isFinite(weekday) || weekday < 0 || weekday > 6) {
      banner.show('星期必須為 0-6', true)
      return
    }
    if (!Number.isFinite(tzOffset)) {
      banner.show('時區偏移必須為整數（分鐘）', true)
      return
    }
    if (!Number.isFinite(timeout) || timeout < 1) {
      banner.show('專案逾時必須 >= 1 秒', true)
      return
    }
    if (!Number.isFinite(concurrency) || concurrency < 1) {
      banner.show('最大並發必須 >= 1', true)
      return
    }
    if (!Number.isFinite(mrPoll)) {
      banner.show('MR 輪詢間隔必須為整數（分鐘；<=0 停用）', true)
      return
    }

    setScheduleSaving(true)
    try {
      await updateSchedule({
        enabled: Boolean(scheduleForm.enabled),
        weekday: Math.trunc(weekday),
        run_time: runTime.length === 4 ? `0${runTime}` : runTime,
        tz_offset_min: Math.trunc(tzOffset),
        per_project_timeout_sec: Math.trunc(timeout),
        max_concurrency: Math.trunc(concurrency),
        mr_poll_interval_min: Math.trunc(mrPoll),
      })
      await loadDashboard()
      banner.show('排程已儲存。影響 cron 的欄位需重啟 reviewer-server；逾時／並發於下一場 run 生效。')
    } catch (error) {
      banner.show(error instanceof Error ? error.message : '儲存排程失敗', true)
    } finally {
      setScheduleSaving(false)
    }
  }

  return (
    <div className="mx-auto flex max-w-[1280px] flex-col gap-5">
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-[28px] font-bold tracking-tight text-ink">控制台</h1>
          <p className="mt-1 text-[13.5px] text-ink-muted">{loading ? '載入中...' : lastRunLine}</p>
        </div>
        <Button variant="primary" onClick={handleRunAll} disabled={activeRunId !== null || loading}>
          {activeRunId !== null ? '執行中...' : '▶ 立即執行'}
        </Button>
      </header>

      {showCatchUp && missed ? (
        <Card className="flex items-center justify-between gap-4 border-warning-border bg-warning-tint p-4">
          <div>
            <div className="text-[14px] font-semibold text-warning-ink">
              錯過週報排程：{missed.label}
            </div>
            <div className="mt-1 text-xs text-warning-ink/80">可立即補跑，或在此分頁稍後提醒。</div>
          </div>
          <div className="flex gap-2">
            <Button onClick={handleCatchUp} disabled={catchUpLoading || activeRunId !== null}>
              {catchUpLoading ? '補跑中...' : '立即補跑'}
            </Button>
            <Button variant="ghost" onClick={handleDismissCatchUp}>
              稍後
            </Button>
          </div>
        </Card>
      ) : null}

      <section className="grid grid-cols-5 gap-3">
        <StatCard label="專案" value={stats.project_count} />
        <StatCard label="工程師" value={stats.person_count} />
        <StatCard label="未讀報告" value={stats.unread_count} valueClassName="text-primary" />
        <StatCard label="待確認" value={stats.pending_count} valueClassName="text-warning-ink" />
        <StatCard
          label="MR 草稿"
          value={stats.mr_draft_count}
          valueClassName="text-mr"
          onClick={() => navigate('/mr-inbox')}
        />
      </section>

      <section className="grid grid-cols-2 gap-5">
        <Card className="p-4">
          <h2 className="mb-3 text-[15px] font-semibold text-ink">最近報告</h2>
          {dashboard?.recent_reports.length ? (
            <div className="overflow-hidden rounded-lg border border-border">
              {dashboard.recent_reports.map((report) => (
                <ListRow key={report.report_id} onClick={() => navigate(`/reports/${report.person_id}`)}>
                  <span className="flex items-center justify-between gap-3">
                    <span>
                      <span className="font-semibold text-ink">{report.person_name}</span>
                      <span className="ml-2 text-xs text-ink-muted">{report.project_name}</span>
                    </span>
                    <StatusPill tone={report.is_read ? 'success' : 'warning'}>
                      {report.is_read ? '已讀' : report.pending_count > 0 ? `待確認 ${report.pending_count}` : '未讀'}
                    </StatusPill>
                  </span>
                </ListRow>
              ))}
            </div>
          ) : (
            <p className="rounded-lg border border-dashed border-border p-6 text-center text-sm text-ink-muted">
              尚無報告
            </p>
          )}
        </Card>

        <Card className="p-4">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-[15px] font-semibold text-ink">最近執行</h2>
            <Button variant="ghost" className="px-2 py-1 text-xs" onClick={() => navigate('/runs')}>
              查看全部
            </Button>
          </div>
          {dashboard?.recent_runs.length ? (
            <div className="overflow-hidden rounded-lg border border-border">
              {dashboard.recent_runs.map((run) => (
                <ListRow key={run.id} onClick={() => navigate(`/runs/${run.id}`)}>
                  <span className="flex items-center justify-between gap-3">
                    <span className="flex items-center gap-2">
                      <StatusPill tone={runStatusTone(run.status)}>{run.status}</StatusPill>
                      <span className="font-medium text-ink">{humanizeTrigger(run.trigger)}</span>
                    </span>
                    <span className="text-xs text-ink-muted">
                      {formatTimestamp(run.started_at)}
                      {formatDurationSuffix(run.duration_sec)}
                    </span>
                  </span>
                </ListRow>
              ))}
            </div>
          ) : (
            <p className="rounded-lg border border-dashed border-border p-6 text-center text-sm text-ink-muted">
              尚無執行紀錄
            </p>
          )}
        </Card>
      </section>

      <Card className="p-5">
        <h2 className="text-[16px] font-semibold text-ink">排程</h2>
        <div className="mt-4 grid grid-cols-2 gap-6">
          <section>
            <h3 className="text-[13.5px] font-semibold text-ink-secondary">週報（軌道 1）</h3>
            <div className="mt-3 grid grid-cols-2 gap-3">
              <label className="flex items-center gap-2 text-[13px] text-ink-secondary">
                <input
                  type="checkbox"
                  checked={Boolean(scheduleForm.enabled)}
                  onChange={(event) =>
                    setScheduleForm((form) => ({ ...form, enabled: event.target.checked }))
                  }
                />
                啟用
              </label>
              <label className="text-[12px] font-medium text-ink-muted">
                星期
                <select
                  className="mt-1 w-full rounded-md border border-border bg-surface px-3 py-2 text-[13.5px] text-ink"
                  value={scheduleForm.weekday ?? 0}
                  onChange={(event) =>
                    setScheduleForm((form) => ({ ...form, weekday: Number(event.target.value) }))
                  }
                >
                  {WEEKDAY_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <LabeledInput
                label="時間（HH:MM）"
                value={scheduleForm.run_time ?? ''}
                onChange={(value) => setScheduleForm((form) => ({ ...form, run_time: value }))}
              />
              <LabeledInput
                label="時區偏移（分）"
                type="number"
                value={scheduleForm.tz_offset_min ?? 480}
                onChange={(value) =>
                  setScheduleForm((form) => ({ ...form, tz_offset_min: Number(value) }))
                }
              />
              <LabeledInput
                label="專案逾時（秒）"
                type="number"
                min={1}
                value={scheduleForm.per_project_timeout_sec ?? 600}
                onChange={(value) =>
                  setScheduleForm((form) => ({ ...form, per_project_timeout_sec: Number(value) }))
                }
              />
              <LabeledInput
                label="最大並發"
                type="number"
                min={1}
                value={scheduleForm.max_concurrency ?? 2}
                onChange={(value) =>
                  setScheduleForm((form) => ({ ...form, max_concurrency: Number(value) }))
                }
              />
            </div>
            <p className="mt-3 text-xs text-ink-muted">
              下次 {dashboard?.schedule.next_run_at ?? '無下次排程'} · {dashboard?.schedule.label ?? '-'} ·{' '}
              {formatDurationLabel(dashboard?.last_run?.duration_sec)}
            </p>
          </section>

          <section>
            <h3 className="text-[13.5px] font-semibold text-ink-secondary">MR 輪詢（軌道 2）</h3>
            <p className="mt-3 text-[13px] text-ink-muted">{dashboard?.schedule.mr_poll_label ?? '-'}</p>
            <div className="mt-3 max-w-xs">
              <LabeledInput
                label="間隔（分鐘，<=0 停用）"
                type="number"
                value={scheduleForm.mr_poll_interval_min ?? 60}
                onChange={(value) =>
                  setScheduleForm((form) => ({ ...form, mr_poll_interval_min: Number(value) }))
                }
              />
            </div>
            <p className="mt-2 text-xs text-ink-muted">{'>=60'} 時須為 60 的倍數（如 60、120）。</p>
          </section>
        </div>
        <div className="mt-5 flex items-center justify-between border-t border-border-subtle pt-4">
          <p className="text-xs text-ink-muted">
            影響 cron 的欄位需重啟 reviewer-server；逾時與並發於下一場 run 即生效。
          </p>
          <Button variant="primary" onClick={handleScheduleSave} disabled={scheduleSaving}>
            {scheduleSaving ? '儲存中...' : '儲存排程'}
          </Button>
        </div>
      </Card>
    </div>
  )
}

function LabeledInput({
  label,
  value,
  onChange,
  type = 'text',
  min,
}: {
  label: string
  value: string | number
  onChange: (value: string) => void
  type?: string
  min?: number
}) {
  return (
    <label className="text-[12px] font-medium text-ink-muted">
      {label}
      <Input
        className="mt-1"
        type={type}
        min={min}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  )
}
