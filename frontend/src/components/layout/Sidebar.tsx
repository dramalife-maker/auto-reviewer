import { NavLink, useLocation, useNavigate } from 'react-router-dom'

import { Badge, NavItem } from '../ui'
import type { Person } from '../../types'

export function Sidebar({
  statusLine,
  mrDraftCount,
  unmatchedCount,
  people,
}: {
  statusLine: string
  mrDraftCount: number
  unmatchedCount: number
  people: Person[]
}) {
  const location = useLocation()
  const navigate = useNavigate()
  const reportsActive = location.pathname.startsWith('/reports')

  return (
    <aside className="flex w-[232px] shrink-0 flex-col border-r border-border bg-surface px-3.5 py-5">
      <div className="mb-3.5 border-b border-border-subtle pb-[18px]">
        <div className="text-[17px] font-bold tracking-tight text-ink">Reviewer</div>
        <div className="mt-1 flex items-center gap-1.5 text-xs text-ink-meta">
          <span className="inline-block h-1.5 w-1.5 rounded-full bg-[#22c55e]" aria-hidden />
          {statusLine}
        </div>
      </div>

      <div className="flex flex-1 flex-col gap-0.5">
        <div className="px-2.5 pb-1 pt-1.5 text-[10.5px] font-bold uppercase tracking-[0.05em] text-ink-faint">
          工作台
        </div>
        <NavLink to="/dashboard">
          {({ isActive }) => <NavItem active={isActive}>控制台</NavItem>}
        </NavLink>
        <NavLink to="/mr-inbox">
          {({ isActive }) => (
            <NavItem
              active={isActive}
              trailing={mrDraftCount > 0 ? <Badge tone="mr">{mrDraftCount}</Badge> : undefined}
            >
              MR 收件匣
            </NavItem>
          )}
        </NavLink>
        <NavLink to="/reports">
          {({ isActive }) => <NavItem active={isActive}>報告閱讀器</NavItem>}
        </NavLink>
        {reportsActive && (
          <div className="ml-2.5 border-l border-border-subtle pl-2.5">
            {people.map((person) => {
              const personActive = location.pathname === `/reports/${person.id}`
              return (
                <button
                  key={person.id}
                  type="button"
                  className={[
                    'mb-0.5 flex w-full items-center justify-between rounded-[6px] px-2 py-[7px] text-left text-[12.5px]',
                    personActive
                      ? 'bg-primary-tint font-semibold text-primary'
                      : 'font-medium text-ink-secondary hover:bg-page',
                  ].join(' ')}
                  onClick={() => navigate(`/reports/${person.id}`)}
                >
                  <span>{person.display_name}</span>
                  {person.open_pending_count > 0 ? (
                    <Badge tone="warning">{person.open_pending_count}</Badge>
                  ) : null}
                </button>
              )
            })}
          </div>
        )}
        <NavLink to="/runs">
          {({ isActive }) => <NavItem active={isActive}>執行紀錄</NavItem>}
        </NavLink>

        <div className="mt-3.5 border-t border-border-subtle px-2.5 pb-1 pt-3.5 text-[10.5px] font-bold uppercase tracking-[0.05em] text-ink-faint">
          設定
        </div>
        <NavLink to="/projects">
          {({ isActive }) => <NavItem active={isActive}>專案設定</NavItem>}
        </NavLink>
        <NavLink to="/people">
          {({ isActive }) => (
            <NavItem
              active={isActive}
              trailing={
                unmatchedCount > 0 ? <Badge tone="warning">{unmatchedCount}</Badge> : undefined
              }
            >
              人員設定
            </NavItem>
          )}
        </NavLink>
      </div>

      <div className="mt-auto border-t border-border-subtle pt-3 text-[11.5px] text-ink-faint">
        Reviewer v0.4 · 內部工具
      </div>
    </aside>
  )
}
