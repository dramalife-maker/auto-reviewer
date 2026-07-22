/** Parse SQLite/API timestamps that are UTC but often lack a zone suffix. */
export function parseUtcTimestamp(value: string): number {
  const normalized = value.includes('T') ? value : value.replace(' ', 'T')
  const utcIso = /(?:Z|[+-]\d{2}:?\d{2})$/i.test(normalized) ? normalized : `${normalized}Z`
  return Date.parse(utcIso)
}

export function formatRunElapsed(startedAt: string | null): string {
  if (!startedAt) {
    return '00:00'
  }
  const started = parseUtcTimestamp(startedAt)
  if (Number.isNaN(started)) {
    return '00:00'
  }
  const elapsedSec = Math.max(0, Math.floor((Date.now() - started) / 1000))
  const minutes = Math.floor(elapsedSec / 60)
  const seconds = elapsedSec % 60
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

export function formatTimestamp(value: string): string {
  const ms = parseUtcTimestamp(value)
  if (Number.isNaN(ms)) {
    return value.length >= 16 ? value.slice(0, 16) : value
  }
  const d = new Date(ms)
  const yyyy = d.getFullYear()
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mi = String(d.getMinutes()).padStart(2, '0')
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}`
}

export function formatDurationSuffix(durationSec: number | null | undefined): string {
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

export function formatDurationLabel(durationSec: number | null | undefined): string {
  if (durationSec == null || durationSec < 0) {
    return '—'
  }
  const minutes = Math.floor(durationSec / 60)
  const seconds = durationSec % 60
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

export function formatReportDateShort(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value)
  if (match) {
    return `${match[2]}-${match[3]}`
  }
  return value.length >= 5 ? value.slice(5) : value
}

export function humanizeTrigger(trigger: string): string {
  switch (trigger) {
    case 'manual_all':
      return '手動全部'
    case 'manual_project':
      return '手動專案'
    case 'manual_person':
      return '手動單人'
    case 'schedule':
      return '週排程'
    case 'mr_poll':
      return 'MR 輪詢'
    case 'manual_mr_poll':
      return '手動 MR 掃描'
    default:
      return trigger
  }
}

export function runStatusTone(
  status: string,
): 'success' | 'warning' | 'danger' | 'neutral' {
  if (status === 'failed' || status === 'skipped_timeout') return 'danger'
  if (status === 'success' || status === 'done') return 'success'
  if (status === 'partial' || status === 'running' || status === 'queued') return 'warning'
  // `cancelled` is neither a failure nor a success — a neutral tone keeps it
  // visually distinct from `failed`.
  if (status === 'cancelled') return 'neutral'
  return 'neutral'
}

export function humanizeProjectError(error: string): string {
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
