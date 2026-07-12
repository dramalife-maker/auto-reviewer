import { useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'

import {
  agentTurnMrReview,
  fetchMrReviews,
  ignoreMrReview,
  publishMrReview,
  updateMrReview,
} from '../api'
import { Button } from '../components/ui/Button.tsx'
import { Card } from '../components/ui/Card.tsx'
import { ListRow } from '../components/ui/ListRow.tsx'
import { StatusPill } from '../components/ui/StatusPill.tsx'
import { Tabs } from '../components/ui/Tabs.tsx'
import { useBanner } from '../context/BannerContext.tsx'
import { formatTimestamp } from '../lib/format'
import type { MrReviewItem, MrReviewStatus } from '../types'

type ChatMessage = {
  role: 'user' | 'assistant'
  text: string
}

const FILTERS: Array<{ id: MrReviewStatus; label: string }> = [
  { id: 'draft', label: '草稿' },
  { id: 'published', label: '已發布' },
  { id: 'ignored', label: '已忽略' },
]

export function MrInboxPage() {
  const [searchParams, setSearchParams] = useSearchParams()
  const status = parseStatus(searchParams.get('status'))
  const { show } = useBanner()
  const [reviews, setReviews] = useState<MrReviewItem[]>([])
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [editorBody, setEditorBody] = useState('')
  const [editorDirty, setEditorDirty] = useState(false)
  const [loading, setLoading] = useState(false)
  const [actionLoading, setActionLoading] = useState(false)
  const [chatInput, setChatInput] = useState('')
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([])
  const [chatLoading, setChatLoading] = useState(false)

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

  useEffect(() => {
    setEditorBody(selected?.draft_body ?? '')
    setEditorDirty(false)
    setChatInput('')
    setChatMessages([])
  }, [selected?.id, selected?.draft_body])

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
      await updateMrReview(selected.id, editorBody)
      setReviews((current) =>
        current.map((review) =>
          review.id === selected.id ? { ...review, draft_body: editorBody } : review,
        ),
      )
      setEditorDirty(false)
      show('草稿已儲存')
    } catch (error) {
      show(error instanceof Error ? error.message : '儲存失敗', true)
    } finally {
      setActionLoading(false)
    }
  }

  async function handlePublish() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }
    if (!window.confirm(`確定要發布 MR !${selected.mr_iid} 的 review？`)) {
      return
    }

    setActionLoading(true)
    try {
      if (editorDirty) {
        await updateMrReview(selected.id, editorBody)
      }
      await publishMrReview(selected.id)
      show(`MR !${selected.mr_iid} 已發布`)
      await reloadAfterAction()
    } catch (error) {
      show(error instanceof Error ? error.message : '發布失敗', true)
    } finally {
      setActionLoading(false)
    }
  }

  async function handleIgnore() {
    if (!selected || selected.status !== 'draft' || actionLoading) {
      return
    }
    if (!window.confirm(`確定要忽略 MR !${selected.mr_iid} 的草稿？`)) {
      return
    }

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

  async function reloadAfterAction() {
    const nextReviews = await fetchMrReviews(status)
    setReviews(nextReviews)
    setSelectedId(nextReviews[0]?.id ?? null)
    setChatMessages([])
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
    } catch (error) {
      show(error instanceof Error ? error.message : '追問失敗', true)
    } finally {
      setChatLoading(false)
    }
  }

  return (
    <div>
      <header className="mb-4">
        <h2 className="text-xl font-bold">MR 收件匣</h2>
        <p className="mt-1 text-sm text-ink-muted">AI 產出的 MR review 草稿，發布前可編輯與追問。</p>
      </header>

      <div className="grid min-h-[560px] grid-cols-[320px_1fr] gap-4">
        <Card className="overflow-hidden">
          <Tabs items={FILTERS} value={status} onChange={handleFilterChange} accent="mr" />
          <div className="max-h-[512px] overflow-y-auto py-2">
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

        <Card className="p-5">
          {selected ? (
            <MrReviewDetail
              actionLoading={actionLoading}
              chatInput={chatInput}
              chatLoading={chatLoading}
              chatMessages={chatMessages}
              editorBody={editorBody}
              editorDirty={editorDirty}
              onAgentTurn={handleAgentTurn}
              onChatInputChange={setChatInput}
              onEditorChange={(value) => {
                setEditorBody(value)
                setEditorDirty(true)
              }}
              onIgnore={handleIgnore}
              onPublish={handlePublish}
              onSave={handleSave}
              review={selected}
            />
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-ink-muted">
              選擇左側 MR review
            </div>
          )}
        </Card>
      </div>
    </div>
  )
}

function MrReviewDetail({
  review,
  editorBody,
  editorDirty,
  actionLoading,
  chatMessages,
  chatInput,
  chatLoading,
  onEditorChange,
  onSave,
  onPublish,
  onIgnore,
  onChatInputChange,
  onAgentTurn,
}: {
  review: MrReviewItem
  editorBody: string
  editorDirty: boolean
  actionLoading: boolean
  chatMessages: ChatMessage[]
  chatInput: string
  chatLoading: boolean
  onEditorChange: (value: string) => void
  onSave: () => void
  onPublish: () => void
  onIgnore: () => void
  onChatInputChange: (value: string) => void
  onAgentTurn: () => void
}) {
  const isDraft = review.status === 'draft'
  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex flex-wrap items-start justify-between gap-3">
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

      <label className="mt-5 text-sm font-semibold" htmlFor="mr-editor">
        {isDraft ? 'Review 草稿' : 'Review 內容（唯讀）'}
      </label>
      <textarea
        id="mr-editor"
        className="mt-2 min-h-[220px] w-full resize-y rounded-lg border border-border bg-page p-3 font-mono text-[13px] leading-6 text-ink-secondary outline-none focus:border-mr"
        onChange={(event) => onEditorChange(event.target.value)}
        readOnly={!isDraft}
        value={editorBody}
      />

      {isDraft ? (
        <div className="mt-3 flex justify-end gap-2">
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

      {isDraft ? (
        <section className="mt-5 border-t border-border-subtle pt-5">
          <h4 className="text-sm font-semibold">Agent Chat</h4>
          <div className="mt-3 max-h-[220px] space-y-3 overflow-y-auto rounded-lg bg-surface">
            {chatMessages.length === 0 ? (
              <p className="rounded-lg bg-page p-3 text-sm text-ink-muted">針對這份 review 向 AI 追問細節。</p>
            ) : (
              chatMessages.map((message, index) => (
                <div
                  key={index}
                  className={[
                    'flex',
                    message.role === 'user' ? 'justify-end' : 'justify-start',
                  ].join(' ')}
                >
                  <div
                    className={[
                      'max-w-[76%] rounded-xl px-3 py-2 text-sm leading-6',
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
          <div className="mt-3 flex gap-2">
            <textarea
              className="min-h-[44px] flex-1 resize-y rounded-lg border border-border bg-surface p-2 text-sm outline-none focus:border-mr"
              disabled={chatLoading || !review.agent_session_id}
              onChange={(event) => onChatInputChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !event.shiftKey) {
                  event.preventDefault()
                  onAgentTurn()
                }
              }}
              placeholder={
                review.agent_session_id ? '例如：為什麼你標記了 transaction helper？' : '此草稿沒有 agent session'
              }
              value={chatInput}
            />
            <Button
              disabled={chatLoading || !review.agent_session_id || chatInput.trim().length === 0}
              onClick={onAgentTurn}
              variant="mr"
            >
              送出
            </Button>
          </div>
        </section>
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
