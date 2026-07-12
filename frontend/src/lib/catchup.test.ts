import { beforeEach, describe, expect, it } from 'vitest'

import {
  CATCHUP_DISMISS_KEY,
  clearCatchUpDismiss,
  dismissCatchUp,
  isCatchUpDismissed,
} from './catchup'

describe('catchup dismiss', () => {
  beforeEach(() => {
    sessionStorage.clear()
  })

  it('hides banner for matching due_at in same tab', () => {
    expect(isCatchUpDismissed('2026-07-06T09:00:00')).toBe(false)
    dismissCatchUp('2026-07-06T09:00:00')
    expect(sessionStorage.getItem(CATCHUP_DISMISS_KEY)).toBe('2026-07-06T09:00:00')
    expect(isCatchUpDismissed('2026-07-06T09:00:00')).toBe(true)
    expect(isCatchUpDismissed('2026-07-13T09:00:00')).toBe(false)
    clearCatchUpDismiss()
    expect(isCatchUpDismissed('2026-07-06T09:00:00')).toBe(false)
  })
})
