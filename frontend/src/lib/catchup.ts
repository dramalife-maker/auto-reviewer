export const CATCHUP_DISMISS_KEY = 'schedule-catchup-dismissed-due'

export function isCatchUpDismissed(dueAt: string | null | undefined): boolean {
  if (!dueAt) return false
  return sessionStorage.getItem(CATCHUP_DISMISS_KEY) === dueAt
}

export function dismissCatchUp(dueAt: string): void {
  sessionStorage.setItem(CATCHUP_DISMISS_KEY, dueAt)
}

export function clearCatchUpDismiss(): void {
  sessionStorage.removeItem(CATCHUP_DISMISS_KEY)
}
