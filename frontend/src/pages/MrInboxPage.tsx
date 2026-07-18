import { useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import { useSearchParams } from 'react-router-dom'
import remarkGfm from 'remark-gfm'

import {
  agentTurnMrReview,
  ApiError,
  fetchMrReviews,
  ignoreMrReview,
  publishMrReview,
  restoreMrReview,
  updateMrReview,
} from '../api'
import { AgentChatLauncher } from '../components/AgentChatLauncher'
import { Button } from '../components/ui/Button.tsx'
import { Card } from '../components/ui/Card.tsx'
import { ConfirmDialog } from '../components/ui/ConfirmDialog.tsx'
import { ListRow } from '../components/ui/ListRow.tsx'
import { StatusPill } from '../components/ui/StatusPill.tsx'
import { Tabs } from '../components/ui/Tabs.tsx'
import { useToast } from '../context/ToastContext.tsx'
import { formatTimestamp } from '../lib/format'
import type { MrReviewDraftConflict, MrReviewItem, MrReviewStatus } from '../types'

type ChatMessage = {
  role: 'user' | 'assistant'
  text: string
}

type EditorMode = 'edit' | 'preview'

const FILTERS: Array<{ id: MrReviewStatus; label: string }> = [
  { id: 'draft', label: '草稿' },
  { id: 'published', label: '已發布' },
  { id: 'ignored', label: '已忽略' },
]

function hydrateChat(review: MrReviewItem | null): ChatMessage[] {
  return (review?.chat_messages ?? []).map((message) => ({
    role: message.role,
    text: message.content,
  }))
}

function parseDraftConflict(error: unknown): MrReviewDraftConflict | null {
  if (!(error instanceof ApiError) || error.status !== 409) {
    return null
  }
  const parsed = error.parseJson()
  if (!parsed || typeof parsed !== 'object') {
    return null
  }
  const record = parsed as Record<string, unknown>
  if (typeof record.draft_body !== 'string' || typeof record.draft_hash !== 'string') {
    return null
  }
  return { draft_body: record.draft_body, draft_hash: record.draft_hash }
}

export function MrInboxPage() {
  const [searchParams, setSearchParams] = useSearchParams()
  const status = parseStatus(searchParams.get('status'))
  const { show } = useToast()
  const [reviews, setReviews] = useState<MrReviewItem[]>([])
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [editorBody, setEditorBody] = useState('')
  const [editorDirty, setEditorDirty] = useState(false)
  const [baselineBody, setBaselineBody] = useState('')
  const [baselineHash, setBaselineHash] = useState('')
  const [draftNewVersion, setDraftNewVersion] = useState(false)
  const [conflict, setConflict] = useState<MrReviewDraftConflict | null>(null)
  const [previewingConflict, setPreviewingConflict] = useState(false)
  const [loading, setLoading] = useState(false)
  const [actionLoading, setActionLoading] = useState(false)
  const [chatInput, setChatInput] = useState('')
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([])
  const [chatLoading, setChatLoading] = useState(false)
  const [listOpen, setListOpen] = useState(true)
  const [pendingConfirm, setPendingConfirm] = useState<'publish' | 'ignore' | null>(null)
  const editorDirtyRef = useRef(false)
  const baselineBodyRef = useRef('')

  useEffect(() => {
    editorDirtyRef.current = editorDirty
  }, [editorDirty])

  useEffect(() => {
    baselineBodyRef.current = baselineBody
  }, [baselineBody])

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    fetchMrReviews(status)
      .then((nextReviews) => {
        if (cancelled) return
        setReviews(nextReviews)
        setSelectedId((current) => {
          if (current !== null && nextReviews.some((review) => review.id === current)) {
            return current
          }
          return nextReviews[0]?.id ?? null
        })
      })
      .catch((error) => {
        if (!cancelled) {
          setReviews([])
          setSelectedId(null)
          show(error instanceof Error ? error.message : '無法載入 MR review', true)
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [show, status])

  const selected = useMemo(
    () => reviews.find((review) => review.id === selectedId) ?? null,
    [reviews, selectedId],
  )

  const showChatPanel = Boolean(
    selected && (selected.status === 'draft' || chatMessages.length > 0),
  )

  // Hydrate editor + chat only when the selected review id changes.
  // While dirty, external draft_body updates must not reset the editor.
  useEffect(() => {
    setEditorBody(selected?.draft_body ?? '')
    setEditorDirty(false)
    setBaselineBody(selected?.draft_body ?? '')
    setBaselineHash(selected?.draft_hash ?? '')
    setDraftNewVersion(false)
    setConflict(null)
    setPreviewingConflict(false)
    setChatInput('')
    setChatMessages(hydrateChat(selected))
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentional: only on id change
  }, [selected?.id])

  function applyServerDraft(nextBody: string, nextHash: string) {
    setEditorBody(nextBody)
    setBaselineBody(nextBody)
    setBaselineHash(nextHash)
    setEditorDirty(false)
    setConflict(null)
    setPreviewingConflict(false)
    setReviews((current) =>
      current.map((review) =>
        review.id === selectedId
          ? { ...review, draft_body: nextBody, draft_hash: nextHash }
          : review,
      ),
    )
  }

  function handleIncomingDraft(nextBody: string, nextHash: string) {
    if (nextBody === baselineBodyRef.current) {
      setBaselineHash(nextHash)
      setReviews((current) =>
        current.map((review) =>
          review.id === selectedId
            ? { ...review, draft_body: nextBody, draft_hash: nextHash }
            : review,
        ),
      )
      return
    }
    if (!editorDirtyRef.current) {
      applyServerDraft(nextBody, nextHash)
      setDraftNewVersion(true)
      return
    }
    setConflict({ draft_body: nextBody, draft_hash: nextHash })
    setPreviewingConflict(false)
  }

  function handleFilterChange(nextStatus: string) {
    const next = parseStatus(nextStatus)
    setSearchParams(next === 'draft' ? {} : { status: next })
    setSelectedId(null)
  }

  function handleSelect(review: MrReviewItem) {
    setSelectedId(review.id)
  }

  async function handleSave() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }

    setActionLoading(true)
    try {
      await updateMrReview(selected.id, editorBody, baselineHash)
      setEditorDirty(false)
      setDraftNewVersion(false)
      show('草稿已儲存')
      const nextReviews = await fetchMrReviews(status)
      setReviews(nextReviews)
      const refreshed = nextReviews.find((review) => review.id === selected.id)
      if (refreshed) {
        setBaselineBody(refreshed.draft_body)
        setBaselineHash(refreshed.draft_hash)
        setEditorBody(refreshed.draft_body)
      }
    } catch (error) {
      const draftConflict = parseDraftConflict(error)
      if (draftConflict) {
        setConflict(draftConflict)
        setPreviewingConflict(false)
        show('磁碟草稿已變更，請選擇如何處理衝突', true)
      } else {
        show(error instanceof Error ? error.message : '儲存失敗', true)
      }
    } finally {
      setActionLoading(false)
    }
  }

  async function handlePublish() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }
    setPendingConfirm('publish')
  }

  async function confirmPublish() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }
    setPendingConfirm(null)
    setActionLoading(true)
    try {
      if (editorDirty) {
        await updateMrReview(selected.id, editorBody, baselineHash)
      }
      await publishMrReview(selected.id)
      show(`MR !${selected.mr_iid} 已發布`)
      await reloadAfterAction()
    } catch (error) {
      const draftConflict = parseDraftConflict(error)
      if (draftConflict) {
        setConflict(draftConflict)
        setPreviewingConflict(false)
        show('磁碟草稿已變更，請選擇如何處理衝突', true)
      } else {
        show(error instanceof Error ? error.message : '發布失敗', true)
      }
    } finally {
      setActionLoading(false)
    }
  }

  async function handleIgnore() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }
    setPendingConfirm('ignore')
  }

  async function confirmIgnore() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }
    setPendingConfirm(null)
    setActionLoading(true)
    try {
      await ignoreMrReview(selected.id)
      show(`已忽略 MR !${selected.mr_iid}`)
      await reloadAfterAction()
    } catch (error) {
      show(error instanceof Error ? error.message : '忽略失敗', true)
    } finally {
      setActionLoading(false)
    }
  }

  async function handleRestore() {
    if (!selected || selected.status !== 'ignored' || actionLoading) {
      return
    }
    setActionLoading(true)
    try {
      await restoreMrReview(selected.id)
      show(`已復原 MR !${selected.mr_iid} 為草稿`)
      setSearchParams({})
      const nextReviews = await fetchMrReviews('draft')
      setReviews(nextReviews)
      setSelectedId(selected.id)
    } catch (error) {
      show(error instanceof Error ? error.message : '復原失敗', true)
    } finally {
      setActionLoading(false)
    }
  }

  async function reloadAfterAction() {
    const nextReviews = await fetchMrReviews(status)
    setReviews(nextReviews)
    setSelectedId(nextReviews[0]?.id ?? null)
  }

  async function handleAgentTurn() {
    const message = chatInput.trim()
    if (!selected || selected.status !== 'draft' || !selected.agent_session_id || !message || chatLoading) {
      return
    }

    setChatMessages((current) => [...current, { role: 'user', text: message }])
    setChatInput('')
    setChatLoading(true)
    try {
      const response = await agentTurnMrReview(selected.id, message)
      setChatMessages((current) => [...current, { role: 'assistant', text: response.reply }])
      setReviews((current) =>
        current.map((review) =>
          review.id === selected.id
            ? {
                ...review,
                agent_session_id: response.agent_session_id,
                draft_body: response.draft_body,
                draft_hash: response.draft_hash,
                chat_messages: [
                  ...review.chat_messages,
                  {
                    id: Date.now(),
                    role: 'user' as const,
                    content: message,
                    created_at: '',
                  },
                  {
                    id: Date.now() + 1,
                    role: 'assistant' as const,
                    content: response.reply,
                    created_at: '',
                  },
                ],
              }
            : review,
        ),
      )
      handleIncomingDraft(response.draft_body, response.draft_hash)
    } catch (error) {
      show(error instanceof Error ? error.message : '追問失敗', true)
    } finally {
      setChatLoading(false)
    }
  }

  return (
    <div className="flex h-[calc(100vh-4rem)] flex-col overflow-hidden">
      <header className="mb-4 shrink-0">
        <h2 className="text-xl font-bold">MR 收件匣</h2>
        <p className="mt-1 text-sm text-ink-muted">AI 產出的 MR review 草稿，發布前可編輯與追問。</p>
      </header>

      <div className="flex min-h-0 flex-1 gap-4 overflow-hidden">
        {listOpen ? (
          <Card className="flex w-[240px] shrink-0 flex-col overflow-hidden">
            <div className="flex shrink-0 items-start gap-1 pr-1">
              <div className="min-w-0 flex-1">
                <Tabs items={FILTERS} value={status} onChange={handleFilterChange} accent="mr" />
              </div>
              <Button
                aria-label="收合收件匣列表"
                className="mt-1 shrink-0 p-1.5"
                onClick={() => setListOpen(false)}
                title="收合收件匣列表"
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
            <div className="min-h-0 flex-1 overflow-y-auto py-2">
              {loading ? (
                <p className="px-3 py-2 text-sm text-ink-muted">載入中...</p>
              ) : reviews.length === 0 ? (
                <p className="px-3 py-2 text-sm text-ink-muted">尚無{statusLabel(status)} MR review</p>
              ) : (
                reviews.map((review) => (
                  <ListRow
                    key={review.id}
                    active={review.id === selectedId}
                    accent="mr"
                    onClick={() => handleSelect(review)}
                  >
                    <span className="block truncate font-semibold">
                      !{review.mr_iid} {review.mr_title ?? `MR !${review.mr_iid}`}
                    </span>
                    <span className="mt-1 block truncate text-xs text-ink-muted">
                      {review.project_name} · !{review.mr_iid} · {review.author_name ?? '未歸戶'}
                    </span>
                    <span className="mt-1 block text-xs text-ink-meta">
                      第 {review.review_round} 輪 · {formatTimestamp(review.created_at)}
                    </span>
                  </ListRow>
                ))
              )}
            </div>
          </Card>
        ) : (
          <Card className="flex w-12 shrink-0 flex-col items-center gap-3 overflow-hidden py-3">
            <Button
              aria-label="展開收件匣列表"
              className="px-2 py-1.5 text-xs"
              onClick={() => setListOpen(true)}
              variant="ghost"
            >
              列表
            </Button>
            {selected ? (
              <span className="px-1 text-center text-xs font-semibold text-mr-dark [writing-mode:vertical-rl]">
                !{selected.mr_iid}
              </span>
            ) : null}
            <span className="text-[11px] text-ink-meta">{reviews.length}</span>
          </Card>
        )}

        <Card className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-5">
          {selected ? (
            <MrReviewDraftPanel
              actionLoading={actionLoading}
              conflict={conflict}
              draftNewVersion={draftNewVersion}
              editorBody={editorBody}
              editorDirty={editorDirty}
              onConflictKeep={() => {
                setConflict(null)
                setPreviewingConflict(false)
              }}
              onConflictLoad={() => {
                if (!conflict) return
                applyServerDraft(conflict.draft_body, conflict.draft_hash)
                setDraftNewVersion(true)
              }}
              onConflictPreview={() => setPreviewingConflict(true)}
              onDismissNewVersion={() => setDraftNewVersion(false)}
              onEditorChange={(value) => {
                setEditorBody(value)
                setEditorDirty(true)
              }}
              onIgnore={handleIgnore}
              onPublish={handlePublish}
              onRestore={handleRestore}
              onSave={handleSave}
              previewingConflict={previewingConflict}
              review={selected}
            />
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-ink-muted">
              {listOpen ? '選擇左側 MR review' : '展開左側列表以選擇 MR review'}
            </div>
          )}
        </Card>

      </div>

      {showChatPanel ? (
        <AgentChatLauncher
          messages={chatMessages}
          input={chatInput}
          loading={chatLoading}
          readOnly={selected!.status !== 'draft'}
          inputDisabled={!selected!.agent_session_id}
          titleSuffix={selected!.status !== 'draft' ? '（唯讀）' : ''}
          emptyHint="針對這份 review 向 AI 追問細節。"
          placeholder={
            selected!.agent_session_id
              ? '例如：為什麼你標記了 transaction helper？'
              : '此草稿沒有 agent session'
          }
          onInputChange={setChatInput}
          onSend={handleAgentTurn}
          panelClassName="h-full max-h-[min(70vh,560px)]"
        />
      ) : null}

      <ConfirmDialog
        open={pendingConfirm === 'ignore'}
        title="忽略草稿"
        message={
          selected
            ? `確定要忽略 MR !${selected.mr_iid} 的草稿？`
            : '確定要忽略這份草稿？'
        }
        confirmLabel="忽略"
        confirmVariant="danger"
        onCancel={() => setPendingConfirm(null)}
        onConfirm={() => {
          void confirmIgnore()
        }}
      />
      <ConfirmDialog
        open={pendingConfirm === 'publish'}
        title="發布 review"
        message={
          selected
            ? `確定要發布 MR !${selected.mr_iid} 的 review？`
            : '確定要發布這份 review？'
        }
        confirmLabel="發布"
        confirmVariant="mr"
        onCancel={() => setPendingConfirm(null)}
        onConfirm={() => {
          void confirmPublish()
        }}
      />
    </div>
  )
}

function MrReviewDraftPanel({
  review,
  editorBody,
  actionLoading,
  draftNewVersion,
  conflict,
  previewingConflict,
  onEditorChange,
  onSave,
  onPublish,
  onIgnore,
  onRestore,
  onDismissNewVersion,
  onConflictPreview,
  onConflictLoad,
  onConflictKeep,
  editorDirty,
}: {
  review: MrReviewItem
  editorBody: string
  editorDirty: boolean
  actionLoading: boolean
  draftNewVersion: boolean
  conflict: MrReviewDraftConflict | null
  previewingConflict: boolean
  onEditorChange: (value: string) => void
  onSave: () => void
  onPublish: () => void
  onIgnore: () => void
  onRestore: () => void
  onDismissNewVersion: () => void
  onConflictPreview: () => void
  onConflictLoad: () => void
  onConflictKeep: () => void
}) {
  const isDraft = review.status === 'draft'
  const isIgnored = review.status === 'ignored'
  const [editorMode, setEditorMode] = useState<EditorMode>(isDraft ? 'edit' : 'preview')

  useEffect(() => {
    setEditorMode(isDraft ? 'edit' : 'preview')
  }, [review.id, isDraft])

  const previewBody = previewingConflict && conflict ? conflict.draft_body : editorBody
  const showingConflictPreview = previewingConflict && conflict !== null

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex shrink-0 flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="truncate text-lg font-bold">
            !{review.mr_iid} {review.mr_title ?? `MR !${review.mr_iid}`}
          </h3>
          <p className="mt-1 text-sm text-ink-muted">
            {review.project_name} · {review.author_name ?? '未歸戶'} · 第 {review.review_round} 輪 ·{' '}
            {statusLabel(review.status)}
          </p>
        </div>
        <StatusPill tone="mr">{review.reviewer_agent}</StatusPill>
      </header>

      <div className="mt-5 flex shrink-0 items-center justify-between gap-3">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <label className="text-sm font-semibold" htmlFor={editorMode === 'edit' && !showingConflictPreview ? 'mr-editor' : undefined}>
            {isDraft ? 'Review 草稿' : 'Review 內容（唯讀）'}
          </label>
          {draftNewVersion ? (
            <span className="inline-flex items-center gap-1 text-xs font-medium text-mr-dark">
              草稿有新版本
              <button type="button" className="underline" onClick={onDismissNewVersion}>
                關閉
              </button>
            </span>
          ) : null}
        </div>
        {isDraft ? (
          <div className="flex rounded-md border border-border p-0.5" role="group" aria-label="編輯模式">
            <button
              type="button"
              aria-pressed={editorMode === 'edit'}
              className={[
                'rounded px-2.5 py-1 text-xs font-semibold',
                editorMode === 'edit' ? 'bg-mr-soft text-mr-dark' : 'text-ink-muted hover:bg-page',
              ].join(' ')}
              onClick={() => setEditorMode('edit')}
            >
              編輯
            </button>
            <button
              type="button"
              aria-pressed={editorMode === 'preview'}
              className={[
                'rounded px-2.5 py-1 text-xs font-semibold',
                editorMode === 'preview' ? 'bg-mr-soft text-mr-dark' : 'text-ink-muted hover:bg-page',
              ].join(' ')}
              onClick={() => setEditorMode('preview')}
            >
              Preview
            </button>
          </div>
        ) : null}
      </div>

      {conflict ? (
        <div
          role="alert"
          className="mt-2 shrink-0 rounded-lg border border-warning-border bg-warning-tint px-3 py-2 text-sm text-warning-ink"
        >
          <p>草稿與伺服器版本衝突。載入會放棄本地編輯；保留後再儲存可能覆寫磁碟上的 agent 改動。</p>
          <div className="mt-2 flex flex-wrap gap-2">
            <Button onClick={onConflictPreview} variant="secondary">
              預覽新版本
            </Button>
            <Button onClick={onConflictLoad} variant="secondary">
              載入新版本
            </Button>
            <Button onClick={onConflictKeep} variant="secondary">
              保留我的編輯
            </Button>
          </div>
        </div>
      ) : null}

      {showingConflictPreview || editorMode === 'preview' ? (
        <div
          aria-label={showingConflictPreview ? '衝突草稿預覽' : 'Markdown 預覽'}
          className="md-preview mt-2 min-h-0 flex-1 overflow-y-auto rounded-lg border border-border bg-page p-4 text-[13.5px] leading-6 text-ink-secondary"
        >
          {previewBody.trim() ? (
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{previewBody}</ReactMarkdown>
          ) : (
            <p className="text-ink-muted">尚無內容</p>
          )}
        </div>
      ) : (
        <textarea
          id="mr-editor"
          className="mt-2 min-h-0 w-full flex-1 resize-none overflow-y-auto rounded-lg border border-border bg-page p-3 font-mono text-[13px] leading-6 text-ink-secondary outline-none focus:border-mr"
          onChange={(event) => onEditorChange(event.target.value)}
          readOnly={!isDraft}
          value={editorBody}
        />
      )}

      {isDraft ? (
        <div className="mt-3 flex shrink-0 justify-end gap-2">
          <Button disabled={actionLoading} onClick={onIgnore} variant="secondary">
            忽略
          </Button>
          <Button disabled={actionLoading || !editorDirty} onClick={onSave} variant="secondary">
            儲存草稿
          </Button>
          <Button disabled={actionLoading} onClick={onPublish} variant="mr">
            發布
          </Button>
        </div>
      ) : null}

      {isIgnored ? (
        <div className="mt-3 flex shrink-0 justify-end gap-2">
          <Button disabled={actionLoading} onClick={onRestore} variant="mr">
            復原為草稿
          </Button>
        </div>
      ) : null}
    </div>
  )
}

function parseStatus(value: string | null): MrReviewStatus {
  if (value === 'published' || value === 'ignored') {
    return value
  }
  return 'draft'
}

function statusLabel(status: MrReviewStatus): string {
  switch (status) {
    case 'draft':
      return '草稿'
    case 'published':
      return '已發布'
    case 'ignored':
      return '已忽略'
  }
}
