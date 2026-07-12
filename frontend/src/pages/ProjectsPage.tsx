import { useEffect, useMemo, useState, type ReactNode } from 'react'

import {
  createProject,
  deleteProject,
  fetchProjects,
  reloadProjects,
  startMrScan,
  startProjectRun,
  updateProject,
} from '../api'
import { useBanner } from '../context/BannerContext'
import { formatReportDateShort, formatRunElapsed } from '../lib/format'
import { sourceIconStyle } from '../lib/icons'
import { Avatar, Button, Card, Input, StatusPill } from '../components/ui'
import type { ProjectInput, ProjectListItem, ProjectUpdateInput, RunStatus } from '../types'

const DEFAULT_MR_SKIP_LABELS = ['wip', 'do-not-review', 'no-ai-review']

type ProjectDraft = {
  name: string
  source_type: 'gitlab' | 'local'
  repo_path: string
  git_remote_url: string
  default_branches: string
  mr_review_skip_labels: string
  mr_review_require_label: string
  isNew: boolean
}

function emptyProjectDraft(): ProjectDraft {
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

function draftFromProject(project: ProjectListItem): ProjectDraft {
  return {
    name: project.name,
    source_type: project.source_type,
    repo_path: project.repo_path,
    git_remote_url: project.git_remote_url ?? '',
    default_branches: (project.default_branches.length > 0
      ? project.default_branches
      : project.default_branch
        ? [project.default_branch]
        : ['main']
    ).join(', '),
    mr_review_skip_labels: (project.mr_review_skip_labels.length > 0
      ? project.mr_review_skip_labels
      : DEFAULT_MR_SKIP_LABELS
    ).join(', '),
    mr_review_require_label: project.mr_review_require_label ?? '',
    isNew: false,
  }
}

function parseCommaList(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
}

function projectPayload(draft: ProjectDraft): ProjectUpdateInput {
  return {
    source_type: draft.source_type,
    repo_path: draft.repo_path.trim(),
    git_remote_url: draft.source_type === 'gitlab' ? draft.git_remote_url.trim() || null : null,
    default_branches: draft.source_type === 'gitlab' ? parseCommaList(draft.default_branches) : [],
    mr_review_skip_labels:
      draft.source_type === 'gitlab' ? parseCommaList(draft.mr_review_skip_labels) : [],
    mr_review_require_label:
      draft.source_type === 'gitlab' ? draft.mr_review_require_label.trim() || null : null,
  }
}

export function ProjectsPage({
  initialProjects,
  activeRun = null,
  onRunStarted,
}: {
  initialProjects?: ProjectListItem[]
  activeRun?: RunStatus | null
  onRunStarted?: (runId: number) => void
}) {
  const { show } = useBanner()
  const [projects, setProjects] = useState<ProjectListItem[]>(initialProjects ?? [])
  const [selectedName, setSelectedName] = useState<string | null>(initialProjects?.[0]?.name ?? null)
  const [draft, setDraft] = useState<ProjectDraft>(() =>
    initialProjects?.[0] ? draftFromProject(initialProjects[0]) : emptyProjectDraft(),
  )
  const [hoveredProject, setHoveredProject] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [localRun, setLocalRun] = useState<{ runId: number; projectName: string; startedAt: string } | null>(
    null,
  )
  const [elapsedTick, setElapsedTick] = useState(0)

  const selectedProject = useMemo(
    () => projects.find((project) => project.name === selectedName) ?? null,
    [projects, selectedName],
  )

  useEffect(() => {
    let cancelled = false
    async function load() {
      try {
        const response = await fetchProjects()
        if (cancelled) return
        setProjects(response.projects)
        const nextSelected =
          selectedName && response.projects.some((project) => project.name === selectedName)
            ? selectedName
            : response.projects[0]?.name ?? null
        setSelectedName(nextSelected)
        setDraft(
          nextSelected
            ? draftFromProject(response.projects.find((project) => project.name === nextSelected)!)
            : emptyProjectDraft(),
        )
      } catch (error) {
        if (!cancelled) show(error instanceof Error ? error.message : '無法載入專案', true)
      }
    }
    if (!initialProjects) void load()
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!localRun && !activeRun) return
    const timer = window.setInterval(() => setElapsedTick((value) => value + 1), 1000)
    return () => window.clearInterval(timer)
  }, [localRun, activeRun])

  async function refreshProjects(preferredName?: string) {
    const response = await fetchProjects()
    setProjects(response.projects)
    const nextSelected =
      preferredName && response.projects.some((project) => project.name === preferredName)
        ? preferredName
        : response.projects[0]?.name ?? null
    setSelectedName(nextSelected)
    setDraft(
      nextSelected
        ? draftFromProject(response.projects.find((project) => project.name === nextSelected)!)
        : emptyProjectDraft(),
    )
  }

  function selectProject(project: ProjectListItem) {
    setSelectedName(project.name)
    setDraft(draftFromProject(project))
  }

  function startNewProject() {
    setSelectedName(null)
    setDraft(emptyProjectDraft())
  }

  function cancelEdit() {
    const fallback = selectedProject ?? projects[0] ?? null
    setSelectedName(fallback?.name ?? null)
    setDraft(fallback ? draftFromProject(fallback) : emptyProjectDraft())
  }

  function getRunningInfo(projectName: string): { startedAt: string | null } | null {
    const activeProject = activeRun?.projects.find((project) => project.name === projectName)
    if (activeProject?.state === 'queued' || activeProject?.state === 'running') {
      return { startedAt: activeRun?.started_at ?? activeProject.started_at }
    }
    if (localRun?.projectName === projectName) {
      void elapsedTick
      return { startedAt: localRun.startedAt }
    }
    return null
  }

  async function handleProjectRun(projectName: string) {
    if (loading || localRun || activeRun) return
    try {
      const response = await startProjectRun(projectName)
      setLocalRun({ runId: response.run_id, projectName, startedAt: new Date().toISOString() })
      onRunStarted?.(response.run_id)
      show(`已啟動 ${projectName} · run #${response.run_id}`)
    } catch (error) {
      show(error instanceof Error ? error.message : '無法啟動執行', true)
    }
  }

  async function handleMrScan(force = false) {
    if (!selectedProject || loading || localRun || activeRun) return
    try {
      const response = await startMrScan(selectedProject.id, force ? { force: true } : undefined)
      setLocalRun({
        runId: response.run_id,
        projectName: selectedProject.name,
        startedAt: new Date().toISOString(),
      })
      onRunStarted?.(response.run_id)
      show(force ? `已啟動 MR 強制重掃 · run #${response.run_id}` : `已啟動 MR 掃描 · run #${response.run_id}`)
    } catch (error) {
      show(error instanceof Error ? error.message : '無法啟動 MR 掃描', true)
    }
  }

  async function handleReloadProjects() {
    if (loading || localRun || activeRun) return
    setLoading(true)
    try {
      const result = await reloadProjects()
      await refreshProjects(selectedName ?? undefined)
      const unhealthyNote = result.unhealthy > 0 ? ` · 異常 ${result.unhealthy}` : ''
      show(`已重新佈建 ${result.total} 個專案（正常 ${result.healthy}${unhealthyNote}）`)
    } catch (error) {
      show(error instanceof Error ? error.message : '重新載入失敗', true)
    } finally {
      setLoading(false)
    }
  }

  async function handleSave() {
    const payload = projectPayload(draft)
    setSaving(true)
    try {
      if (draft.isNew) {
        const name = draft.name.trim()
        if (!name) throw new Error('請輸入專案名稱')
        const created = await createProject({ name, ...payload } satisfies ProjectInput)
        await refreshProjects(created.name)
        show(`已新增專案 ${created.name}`)
      } else {
        const updated = await updateProject(draft.name, payload)
        await refreshProjects(updated.name)
        show(`已儲存專案 ${updated.name}`)
      }
    } catch (error) {
      show(error instanceof Error ? error.message : '儲存失敗', true)
    } finally {
      setSaving(false)
    }
  }

  async function handleDelete() {
    if (draft.isNew) return
    if (!window.confirm(`確定要移除專案「${draft.name}」？`)) return
    try {
      await deleteProject(draft.name)
      await refreshProjects()
      show(`已移除專案 ${draft.name}`)
    } catch (error) {
      show(error instanceof Error ? error.message : '移除失敗', true)
    }
  }

  const selectedRunning = !draft.isNew ? getRunningInfo(draft.name) : null
  const actionDisabled = loading || saving || Boolean(localRun || activeRun)

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold tracking-tight text-ink">專案設定</h1>
        <Button onClick={handleReloadProjects} disabled={actionDisabled}>
          {loading ? '載入中...' : '重新載入'}
        </Button>
      </div>

      <div className="flex h-[calc(100vh-9rem)] gap-4 overflow-hidden">
        <Card className="flex h-full w-[260px] shrink-0 flex-col overflow-hidden">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <span className="font-semibold">專案</span>
            <Button className="px-3 py-1.5" onClick={startNewProject} disabled={actionDisabled}>
              +
            </Button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto py-2">
            {projects.length === 0 ? (
              <p className="px-4 py-3 text-sm text-ink-muted">尚無專案</p>
            ) : (
              projects.map((project) => {
                const active = !draft.isNew && selectedName === project.name
                const runningInfo = getRunningInfo(project.name)
                const showRunButton = hoveredProject === project.name && !runningInfo
                const dateLabel = project.last_report_date
                  ? formatReportDateShort(project.last_report_date)
                  : ''
                return (
                  <div
                    key={project.name}
                    className={[
                      'flex items-center px-2',
                      active ? 'bg-primary-tint shadow-[inset_3px_0_0_#4f46e5]' : 'hover:bg-page',
                    ].join(' ')}
                    onMouseEnter={() => setHoveredProject(project.name)}
                    onMouseLeave={() => setHoveredProject(null)}
                  >
                    <button
                      type="button"
                      className="flex min-w-0 flex-1 items-center gap-2 py-2.5 text-left"
                      onClick={() => selectProject(project)}
                    >
                      <span
                        className="shrink-0 bg-center bg-no-repeat"
                        style={sourceIconStyle(project.source_type, active)}
                        aria-hidden
                      />
                      <span className="truncate text-[13.5px] font-medium">{project.name}</span>
                    </button>
                    <div className="flex h-[22px] w-[74px] shrink-0 items-center justify-end text-xs">
                      {runningInfo ? (
                        <span className="rounded-full bg-warning-tint px-2 py-0.5 text-warning-ink">
                          {formatRunElapsed(runningInfo.startedAt)}
                        </span>
                      ) : showRunButton ? (
                        <button
                          type="button"
                          className="h-[22px] rounded bg-primary px-2 text-[12px] font-semibold text-white disabled:opacity-50"
                          disabled={actionDisabled}
                          onClick={(event) => {
                            event.stopPropagation()
                            void handleProjectRun(project.name)
                          }}
                        >
                          ▶ 執行
                        </button>
                      ) : (
                        <span className="h-[22px] leading-[22px] text-ink-meta">{dateLabel}</span>
                      )}
                    </div>
                  </div>
                )
              })
            )}
          </div>
        </Card>

        <Card className="flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          <div className="flex shrink-0 items-start justify-between gap-3 border-b border-border px-5 py-4">
            <div className="flex min-w-0 items-center gap-2">
              <span
                className="shrink-0 bg-center bg-no-repeat"
                style={sourceIconStyle(draft.source_type, true)}
                aria-hidden
              />
              <h2 className="truncate text-lg font-bold">{draft.isNew ? '新增專案' : draft.name}</h2>
              {selectedRunning && <StatusPill tone="warning">執行中</StatusPill>}
            </div>
            {!draft.isNew && (
              <div className="flex shrink-0 flex-wrap justify-end gap-2">
                {selectedProject?.is_git_repo ? (
                  <>
                    <Button onClick={() => void handleMrScan()} disabled={actionDisabled}>
                      掃描 MR
                    </Button>
                    <Button onClick={() => void handleMrScan(true)} disabled={actionDisabled}>
                      強制重掃
                    </Button>
                  </>
                ) : null}
                <Button variant="danger" onClick={() => void handleDelete()} disabled={actionDisabled}>
                  移除
                </Button>
              </div>
            )}
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {selectedProject?.health === 'unhealthy' && selectedProject.health_reason ? (
              <p className="mb-4 rounded-md border border-warning-border bg-warning-tint px-3 py-2 text-sm text-warning-ink">
                狀態異常：{selectedProject.health_reason}
              </p>
            ) : null}

            <div className="grid gap-4">
              {draft.isNew && (
                <Field label="專案名稱" required>
                  <Input
                    value={draft.name}
                    placeholder="game-backend"
                    onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                  />
                </Field>
              )}

              <Field label="來源類型">
                <div className="flex flex-wrap gap-2">
                  {(['gitlab', 'local'] as const).map((sourceType) => (
                    <button
                      key={sourceType}
                      type="button"
                      className={[
                        'rounded-full border px-3 py-1.5 text-[13px] font-semibold',
                        draft.source_type === sourceType
                          ? 'border-primary bg-primary-tint text-primary'
                          : 'border-border bg-surface text-ink-secondary',
                      ].join(' ')}
                      onClick={() => setDraft({ ...draft, source_type: sourceType })}
                    >
                      {sourceType === 'gitlab' ? 'GitLab' : '本地'}
                    </button>
                  ))}
                </div>
              </Field>

              {draft.source_type === 'gitlab' && (
                <>
                  <Field label="Git Remote URL" required>
                    <Input
                      className="font-mono"
                      value={draft.git_remote_url}
                      placeholder="git@gitlab.example.com:team/repo.git"
                      onChange={(event) => setDraft({ ...draft, git_remote_url: event.target.value })}
                    />
                  </Field>
                  <Field label="常駐分支" required hint="啟動時會為這些分支建立 worktree，週報預設看第一個分支。">
                    <Input
                      className="font-mono"
                      value={draft.default_branches}
                      placeholder="main, develop"
                      onChange={(event) => setDraft({ ...draft, default_branches: event.target.value })}
                    />
                  </Field>
                  <div>
                    <span className="mb-1.5 block text-[13px] font-semibold text-ink-secondary">
                      MR 標籤
                    </span>
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      <Field label="排除" hint="逗號分隔。帶有任一標籤的 MR 不會進入 AI review。">
                        <Input
                          className="font-mono"
                          value={draft.mr_review_skip_labels}
                          placeholder="wip, do-not-review, no-ai-review"
                          onChange={(event) =>
                            setDraft({ ...draft, mr_review_skip_labels: event.target.value })
                          }
                        />
                      </Field>
                      <Field label="必備（可選）" hint="設定後，只有帶此標籤的 MR 才會被掃描。">
                        <Input
                          className="font-mono"
                          value={draft.mr_review_require_label}
                          placeholder="ready-for-review"
                          onChange={(event) =>
                            setDraft({ ...draft, mr_review_require_label: event.target.value })
                          }
                        />
                      </Field>
                    </div>
                  </div>
                </>
              )}

              <Field label="儲存路徑" required>
                <Input
                  className="font-mono"
                  value={draft.repo_path}
                  placeholder="game-backend"
                  onChange={(event) => setDraft({ ...draft, repo_path: event.target.value })}
                />
              </Field>

              <Field label="工程師對應">
                <div className="space-y-2">
                  {selectedProject && selectedProject.engineers.length > 0 ? (
                    selectedProject.engineers.map((engineer) => (
                      <div
                        key={`${engineer.gitlab_username ?? ''}-${engineer.display_name}`}
                        className="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2 text-[13px]"
                      >
                        <Avatar name={engineer.display_name} size={28} />
                        <span className="font-mono text-ink-muted">{engineer.gitlab_username ?? '—'}</span>
                        <span className="text-ink-meta" aria-hidden>
                          →
                        </span>
                        <span className="font-medium">{engineer.display_name}</span>
                      </div>
                    ))
                  ) : (
                    <p className="text-sm text-ink-muted">尚無工程師對應（執行 review 後會依 commit 歸戶）</p>
                  )}
                </div>
              </Field>
            </div>
          </div>

          <div className="flex shrink-0 justify-end gap-2 border-t border-border px-5 py-4">
            <Button onClick={cancelEdit}>取消</Button>
            <Button variant="primary" onClick={() => void handleSave()} disabled={saving}>
              {saving ? '儲存中...' : '儲存'}
            </Button>
          </div>
        </Card>
      </div>
    </div>
  )
}

function Field({
  label,
  required = false,
  hint,
  children,
}: {
  label: string
  required?: boolean
  hint?: string
  children: ReactNode
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[13px] font-semibold text-ink-secondary">
        {label} {required && <span className="text-danger">*</span>}
      </span>
      {children}
      {hint && <span className="mt-1 block text-xs text-ink-muted">{hint}</span>}
    </label>
  )
}
