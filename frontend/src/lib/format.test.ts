import { describe, expect, it, vi } from 'vitest'

import { formatRunElapsed, parseUtcTimestamp } from './format'

describe('parseUtcTimestamp', () => {
  it('treats zone-less sqlite strings as UTC', () => {
    expect(parseUtcTimestamp('2026-07-12 06:03:00')).toBe(Date.parse('2026-07-12T06:03:00Z'))
  })
})

describe('formatRunElapsed', () => {
  it('computes elapsed from UTC started_at without local offset skew', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-07-12T06:04:00Z'))
    expect(formatRunElapsed('2026-07-12 06:03:00')).toBe('01:00')
    vi.useRealTimers()
  })
})
