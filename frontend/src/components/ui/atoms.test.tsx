import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { Avatar } from './Avatar'
import { Badge } from './Badge'
import { Button } from './Button'
import { Card } from './Card'
import { Input } from './Input'
import { ListRow } from './ListRow'
import { NavItem } from './NavItem'
import { StatCard } from './StatCard'
import { StatusPill } from './StatusPill'
import { Tabs } from './Tabs'

describe('ui atoms', () => {
  it('Button primary has indigo background and no shadow class', () => {
    render(<Button variant="primary">立即執行</Button>)
    const btn = screen.getByRole('button', { name: '立即執行' })
    expect(btn.className).toContain('bg-primary')
    expect(btn.className).not.toMatch(/shadow(?!-\[)/)
  })

  it('Badge mr uses violet track color', () => {
    render(<Badge tone="mr">3</Badge>)
    expect(screen.getByText('3').className).toContain('bg-mr')
  })

  it('NavItem active uses primary tint', () => {
    render(<NavItem active>控制台</NavItem>)
    expect(screen.getByRole('button', { name: '控制台' }).className).toContain('bg-primary-tint')
  })

  it('StatCard renders label and value', () => {
    render(<StatCard label="專案" value={12} />)
    expect(screen.getByText('專案')).toBeInTheDocument()
    expect(screen.getByText('12')).toBeInTheDocument()
  })

  it('ListRow active applies inset selection accent', () => {
    render(
      <ListRow active accent="primary">
        Run #1
      </ListRow>,
    )
    expect(screen.getByText('Run #1').className).toContain('shadow-[inset_3px_0_0_')
  })

  it('Tabs marks active tab with inset underline', () => {
    render(
      <Tabs
        items={[
          { id: 'a', label: '總覽' },
          { id: 'b', label: '成長趨勢' },
        ]}
        value="a"
        onChange={() => {}}
      />,
    )
    expect(screen.getByRole('tab', { name: '總覽' }).className).toContain('shadow-[inset_0_-2px_0_')
  })

  it('StatusPill success uses green tint', () => {
    render(<StatusPill tone="success">已讀</StatusPill>)
    expect(screen.getByText('已讀').className).toContain('bg-success-tint')
  })

  it('Avatar shows initial', () => {
    render(<Avatar name="Alice Wu" />)
    expect(screen.getByText('A')).toBeInTheDocument()
  })

  it('Card and Input render without drop-shadow utilities', () => {
    const { container } = render(
      <Card className="p-2">
        <Input aria-label="名稱" />
      </Card>,
    )
    const card = container.firstElementChild
    expect(card?.className).toContain('rounded-xl')
    expect(card?.className).not.toMatch(/(?:^|\s)shadow(?:-[a-z]+)?(?:\s|$)/)
  })
})
