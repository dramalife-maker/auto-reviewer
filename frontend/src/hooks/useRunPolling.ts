import { useCallback, useEffect, useRef, useState } from 'react'

import { fetchRun } from '../api'
import type { RunStatus } from '../types'

const TERMINAL_STATUSES = new Set(['success', 'partial', 'failed'])

export function useRunPolling({
  onComplete,
}: {
  onComplete?: (run: RunStatus) => void | Promise<void>
} = {}) {
  const [activeRunId, setActiveRunId] = useState<number | null>(null)
  const [activeRun, setActiveRun] = useState<RunStatus | null>(null)
  const timerRef = useRef<number | null>(null)
  const onCompleteRef = useRef(onComplete)

  useEffect(() => {
    onCompleteRef.current = onComplete
  }, [onComplete])

  const stopPolling = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const pollRun = useCallback(
    async (runId: number) => {
      const run = await fetchRun(runId)
      setActiveRun(run)

      if (!TERMINAL_STATUSES.has(run.status)) {
        return
      }

      stopPolling()
      setActiveRunId(null)
      setActiveRun(null)
      await onCompleteRef.current?.(run)
    },
    [stopPolling],
  )

  const startPolling = useCallback(
    (runId: number) => {
      stopPolling()
      setActiveRunId(runId)
      setActiveRun(null)
      void pollRun(runId)
      timerRef.current = window.setInterval(() => {
        void pollRun(runId)
      }, 2000)
    },
    [pollRun, stopPolling],
  )

  useEffect(() => stopPolling, [stopPolling])

  return { startPolling, activeRunId, activeRun }
}
