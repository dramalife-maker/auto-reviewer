import { useEffect, useMemo, useState, type ReactNode } from 'react'

import {
  bindIdentity,
  createPerson,
  fetchPeople,
  fetchPersonDetail,
  fetchUnmatchedAuthors,
  renamePerson,
  unbindIdentity,
} from '../api'
import { useBanner } from '../context/BannerContext'
import { Avatar, Button, Card, Input } from '../components/ui'
import type { IdentityKind, Person, PersonDetail, UnmatchedAuthor } from '../types'

const IDENTITY_KINDS: { value: IdentityKind; label: string }[] = [
  { value: 'git_email', label: 'git email' },
  { value: 'gitlab_user', label: 'gitlab user' },
  { value: 'glab_user', label: 'glab user' },
]

type NewNameByAuthor = Record<number, string>
type SelectedPersonByAuthor = Record<number, string>

export function PeoplePage({
  initialPeople,
  initialUnmatchedAuthors,
}: {
  initialPeople?: Person[]
  initialUnmatchedAuthors?: UnmatchedAuthor[]
}) {
  const { show } = useBanner()
  const [people, setPeople] = useState<Person[]>(initialPeople ?? [])
  const [unmatchedAuthors, setUnmatchedAuthors] = useState<UnmatchedAuthor[]>(
    initialUnmatchedAuthors ?? [],
  )
  const [selectedPersonId, setSelectedPersonId] = useState<number | null>(
    initialPeople?.[0]?.id ?? null,
  )
  const [personDetail, setPersonDetail] = useState<PersonDetail | null>(null)
  const [displayNameDraft, setDisplayNameDraft] = useState('')
  const [identityKind, setIdentityKind] = useState<IdentityKind>('git_email')
  const [identityValue, setIdentityValue] = useState('')
  const [isCreatingPerson, setIsCreatingPerson] = useState(false)
  const [newPersonName, setNewPersonName] = useState('')
  const [newNameByAuthor, setNewNameByAuthor] = useState<NewNameByAuthor>({})
  const [selectedPersonByAuthor, setSelectedPersonByAuthor] = useState<SelectedPersonByAuthor>({})
  const [saving, setSaving] = useState(false)

  const selectedPerson = useMemo(
    () => people.find((person) => person.id === selectedPersonId) ?? null,
    [people, selectedPersonId],
  )

  useEffect(() => {
    let cancelled = false
    async function load() {
      try {
        const [peopleResponse, unmatchedResponse] = await Promise.all([
          fetchPeople(),
          fetchUnmatchedAuthors(),
        ])
        if (cancelled) return
        setPeople(peopleResponse)
        setUnmatchedAuthors(unmatchedResponse)
        setSelectedPersonId((current) => {
          if (current && peopleResponse.some((person) => person.id === current)) return current
          return peopleResponse[0]?.id ?? null
        })
      } catch (error) {
        if (!cancelled) show(error instanceof Error ? error.message : '無法載入人員設定', true)
      }
    }
    if (!initialPeople || !initialUnmatchedAuthors) void load()
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    async function loadDetail(personId: number) {
      try {
        const detail = await fetchPersonDetail(personId)
        if (cancelled) return
        setPersonDetail(detail)
        setDisplayNameDraft(detail.display_name)
      } catch (error) {
        if (!cancelled) {
          setPersonDetail(null)
          show(error instanceof Error ? error.message : '無法載入人員詳情', true)
        }
      }
    }
    if (selectedPersonId === null || isCreatingPerson) {
      setPersonDetail(null)
      setDisplayNameDraft('')
      return
    }
    void loadDetail(selectedPersonId)
    return () => {
      cancelled = true
    }
  }, [selectedPersonId, isCreatingPerson])

  async function reloadPeople(nextSelectedId = selectedPersonId) {
    const [peopleResponse, unmatchedResponse] = await Promise.all([
      fetchPeople(),
      fetchUnmatchedAuthors(),
    ])
    setPeople(peopleResponse)
    setUnmatchedAuthors(unmatchedResponse)
    const validSelected =
      nextSelectedId && peopleResponse.some((person) => person.id === nextSelectedId)
        ? nextSelectedId
        : peopleResponse[0]?.id ?? null
    setSelectedPersonId(validSelected)
    if (validSelected) {
      const detail = await fetchPersonDetail(validSelected)
      setPersonDetail(detail)
      setDisplayNameDraft(detail.display_name)
    }
  }

  function selectPerson(personId: number) {
    setIsCreatingPerson(false)
    setIdentityKind('git_email')
    setIdentityValue('')
    setSelectedPersonId(personId)
  }

  async function handleCreatePerson(displayName: string) {
    const trimmed = displayName.trim()
    if (!trimmed || saving) return
    setSaving(true)
    try {
      const created = await createPerson(trimmed)
      setIsCreatingPerson(false)
      setNewPersonName('')
      await reloadPeople(created.id)
      show(`已建立 ${created.display_name}`)
    } catch (error) {
      show(error instanceof Error ? error.message : '建立人員失敗', true)
    } finally {
      setSaving(false)
    }
  }

  async function handleRenamePerson() {
    if (selectedPersonId === null || saving) return
    const trimmed = displayNameDraft.trim()
    if (!trimmed) {
      show('顯示名不可為空', true)
      return
    }
    setSaving(true)
    try {
      const detail = await renamePerson(selectedPersonId, trimmed)
      setPersonDetail(detail)
      setDisplayNameDraft(detail.display_name)
      await reloadPeople(selectedPersonId)
      show('已更新顯示名')
    } catch (error) {
      show(error instanceof Error ? error.message : '更名失敗', true)
    } finally {
      setSaving(false)
    }
  }

  async function handleBindSettingsIdentity() {
    if (selectedPersonId === null || saving) return
    const trimmed = identityValue.trim()
    if (!trimmed) {
      show('identity value 不可為空', true)
      return
    }
    if (personDetail?.identities.some((item) => item.kind === identityKind && item.value === trimmed)) {
      show('此 identity 已綁定')
      return
    }
    setSaving(true)
    try {
      await bindIdentity(selectedPersonId, identityKind, trimmed)
      setIdentityValue('')
      await reloadPeople(selectedPersonId)
      show('已新增 identity')
    } catch (error) {
      show(error instanceof Error ? error.message : '綁定失敗', true)
    } finally {
      setSaving(false)
    }
  }

  async function handleUnbindIdentity(identityId: number) {
    if (selectedPersonId === null || saving) return
    setSaving(true)
    try {
      await unbindIdentity(selectedPersonId, identityId)
      await reloadPeople(selectedPersonId)
      show('已移除 identity')
    } catch (error) {
      show(error instanceof Error ? error.message : '移除失敗', true)
    } finally {
      setSaving(false)
    }
  }

  async function handleBindExisting(author: UnmatchedAuthor) {
    const personId = Number(selectedPersonByAuthor[author.id])
    if (!personId) return
    setSaving(true)
    try {
      await bindIdentity(personId, author.kind, author.value)
      await reloadPeople(personId)
      show(`已將 ${author.value} 綁定到現有人員`)
    } catch (error) {
      show(error instanceof Error ? error.message : '綁定失敗', true)
    } finally {
      setSaving(false)
    }
  }

  async function handleBindNew(author: UnmatchedAuthor) {
    const displayName = (newNameByAuthor[author.id] ?? '').trim()
    if (!displayName) return
    setSaving(true)
    try {
      const person = await createPerson(displayName)
      await bindIdentity(person.id, author.kind, author.value)
      await reloadPeople(person.id)
      show(`已建立 ${displayName} 並綁定 ${author.value}`)
    } catch (error) {
      show(error instanceof Error ? error.message : '建立或綁定失敗', true)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-bold tracking-tight text-ink">人員設定</h1>

      <UnmatchedSection
        authors={unmatchedAuthors}
        people={people}
        selectedPersonByAuthor={selectedPersonByAuthor}
        newNameByAuthor={newNameByAuthor}
        saving={saving}
        onSelectPerson={(authorId, personId) =>
          setSelectedPersonByAuthor((current) => ({ ...current, [authorId]: personId }))
        }
        onNewName={(authorId, value) =>
          setNewNameByAuthor((current) => ({ ...current, [authorId]: value }))
        }
        onBindExisting={(author) => void handleBindExisting(author)}
        onBindNew={(author) => void handleBindNew(author)}
      />

      <div className="flex min-h-[620px] gap-4">
        <Card className="flex w-[260px] shrink-0 flex-col overflow-hidden">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <span className="font-semibold">人員</span>
            <Button
              className="px-3 py-1.5"
              onClick={() => {
                setIsCreatingPerson(true)
                setNewPersonName('')
                setSelectedPersonId(null)
              }}
            >
              +
            </Button>
          </div>
          <div className="flex-1 overflow-y-auto py-2">
            {people.length === 0 ? (
              <p className="px-4 py-3 text-sm text-ink-muted">尚無人員</p>
            ) : (
              people.map((person) => (
                <button
                  key={person.id}
                  type="button"
                  className={[
                    'block w-full px-4 py-3 text-left text-[13.5px]',
                    !isCreatingPerson && person.id === selectedPersonId
                      ? 'bg-primary-tint font-semibold text-primary shadow-[inset_3px_0_0_#4f46e5]'
                      : 'text-ink-secondary hover:bg-page',
                  ].join(' ')}
                  onClick={() => selectPerson(person.id)}
                >
                  <span className="block truncate">{person.display_name}</span>
                  <span className="mt-0.5 block text-xs text-ink-meta">
                    {person.identity_count} identities
                  </span>
                </button>
              ))
            )}
          </div>
        </Card>

        <Card className="min-w-0 flex-1 p-5">
          {isCreatingPerson ? (
            <div className="max-w-xl space-y-4">
              <h2 className="text-lg font-bold">新增人員</h2>
              <Field label="顯示名" required>
                <Input
                  value={newPersonName}
                  placeholder="Alice Chen"
                  onChange={(event) => setNewPersonName(event.target.value)}
                />
              </Field>
              <div className="flex justify-end gap-2">
                <Button
                  onClick={() => {
                    setIsCreatingPerson(false)
                    setSelectedPersonId(people[0]?.id ?? null)
                  }}
                >
                  取消
                </Button>
                <Button
                  variant="primary"
                  disabled={saving}
                  onClick={() => void handleCreatePerson(newPersonName)}
                >
                  建立
                </Button>
              </div>
            </div>
          ) : personDetail ? (
            <div className="space-y-5">
              <div className="flex items-center gap-3">
                <Avatar name={personDetail.display_name} />
                <div className="min-w-0 flex-1">
                  <label className="sr-only" htmlFor="people-display-name">
                    顯示名
                  </label>
                  <Input
                    id="people-display-name"
                    value={displayNameDraft}
                    onChange={(event) => setDisplayNameDraft(event.target.value)}
                  />
                </div>
                <Button variant="primary" disabled={saving} onClick={() => void handleRenamePerson()}>
                  儲存
                </Button>
              </div>
              <p className="text-xs text-ink-muted">
                更名會同步 rename reports/_people/{'{顯示名}'}/；專案層報告目錄不會搬移。
              </p>

              <Field label="Identities">
                <div className="space-y-2">
                  {personDetail.identities.length > 0 ? (
                    personDetail.identities.map((identity) => (
                      <div
                        key={identity.id}
                        className="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2 text-[13px]"
                      >
                        <span className="rounded bg-page px-2 py-1 font-mono text-xs text-ink-muted">
                          {identity.kind}
                        </span>
                        <span className="font-mono">{identity.value}</span>
                        {identity.label && <span className="text-ink-meta">{identity.label}</span>}
                        <button
                          type="button"
                          className="ml-auto text-danger hover:underline disabled:opacity-50"
                          disabled={saving}
                          onClick={() => void handleUnbindIdentity(identity.id)}
                        >
                          移除
                        </button>
                      </div>
                    ))
                  ) : (
                    <p className="text-sm text-ink-muted">尚無 identity；未歸戶 commit 會進入佇列。</p>
                  )}
                  <div className="flex flex-wrap gap-2">
                    <select
                      className="rounded-md border border-border bg-surface px-3 py-2 text-[13.5px]"
                      aria-label="identity kind"
                      value={identityKind}
                      onChange={(event) => setIdentityKind(event.target.value as IdentityKind)}
                    >
                      {IDENTITY_KINDS.map((kind) => (
                        <option key={kind.value} value={kind.value}>
                          {kind.label}
                        </option>
                      ))}
                    </select>
                    <Input
                      className="min-w-[220px] flex-1 font-mono"
                      value={identityValue}
                      placeholder="value"
                      onChange={(event) => setIdentityValue(event.target.value)}
                    />
                    <Button
                      variant="primary"
                      disabled={saving}
                      onClick={() => void handleBindSettingsIdentity()}
                    >
                      新增
                    </Button>
                  </div>
                </div>
              </Field>

              <Field label="參與專案">
                {personDetail.projects.length > 0 ? (
                  <ul className="list-disc space-y-1 pl-5 text-[13.5px]">
                    {personDetail.projects.map((project) => (
                      <li key={project.id}>{project.name}</li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-sm text-ink-muted">尚無參與專案（來自報告或 participation）</p>
                )}
              </Field>
            </div>
          ) : (
            <p className="text-sm text-ink-muted">
              {selectedPerson ? '載入人員詳情中...' : '選擇左側人員，或新增一位人員。'}
            </p>
          )}
        </Card>
      </div>
    </div>
  )
}

function UnmatchedSection({
  authors,
  people,
  selectedPersonByAuthor,
  newNameByAuthor,
  saving,
  onSelectPerson,
  onNewName,
  onBindExisting,
  onBindNew,
}: {
  authors: UnmatchedAuthor[]
  people: Person[]
  selectedPersonByAuthor: SelectedPersonByAuthor
  newNameByAuthor: NewNameByAuthor
  saving: boolean
  onSelectPerson: (authorId: number, personId: string) => void
  onNewName: (authorId: number, value: string) => void
  onBindExisting: (author: UnmatchedAuthor) => void
  onBindNew: (author: UnmatchedAuthor) => void
}) {
  if (authors.length === 0) return null

  return (
    <Card className="p-4">
      <div className="mb-3">
        <h2 className="text-base font-bold">未歸戶作者</h2>
        <p className="mt-1 text-sm text-ink-muted">
          未綁定 git email 不會產出週報。綁定後請重新執行 review。
        </p>
      </div>
      <div className="space-y-3">
        {authors.map((author) => (
          <article key={author.id} className="rounded-lg border border-border p-3">
            <div className="mb-3 flex flex-wrap items-baseline gap-x-2 gap-y-1">
              <strong className="font-mono">{author.value}</strong>
              <span className="text-sm text-ink-muted">
                {author.project_name ?? '未知專案'} · {author.commit_count} commits
              </span>
            </div>
            <div className="flex flex-wrap gap-2">
              <Input
                className="min-w-[180px] flex-1"
                aria-label={`新顯示名稱 ${author.value}`}
                placeholder="新顯示名稱"
                value={newNameByAuthor[author.id] ?? ''}
                onChange={(event) => onNewName(author.id, event.target.value)}
              />
              <Button disabled={saving} onClick={() => onBindNew(author)}>
                建立並綁定
              </Button>
              <select
                className="min-w-[180px] rounded-md border border-border bg-surface px-3 py-2 text-[13.5px]"
                aria-label={`綁定到現有人員 ${author.value}`}
                value={selectedPersonByAuthor[author.id] ?? ''}
                onChange={(event) => onSelectPerson(author.id, event.target.value)}
              >
                <option value="">綁定到現有人員</option>
                {people.map((person) => (
                  <option key={person.id} value={person.id}>
                    {person.display_name}
                  </option>
                ))}
              </select>
              <Button disabled={saving} onClick={() => onBindExisting(author)}>
                綁定
              </Button>
            </div>
          </article>
        ))}
      </div>
    </Card>
  )
}

function Field({
  label,
  required = false,
  children,
}: {
  label: string
  required?: boolean
  children: ReactNode
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[13px] font-semibold text-ink-secondary">
        {label} {required && <span className="text-danger">*</span>}
      </span>
      {children}
    </label>
  )
}
