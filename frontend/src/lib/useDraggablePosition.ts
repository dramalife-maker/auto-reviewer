import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'

export type DragPosition = {
  right: number
  bottom: number
}

const DRAG_THRESHOLD_PX = 4

function readStoredPosition(storageKey: string): DragPosition | null {
  try {
    const raw = window.localStorage.getItem(storageKey)
    if (!raw) return null
    const parsed = JSON.parse(raw) as unknown
    if (
      typeof parsed === 'object' &&
      parsed !== null &&
      typeof (parsed as DragPosition).right === 'number' &&
      typeof (parsed as DragPosition).bottom === 'number'
    ) {
      return parsed as DragPosition
    }
    return null
  } catch {
    return null
  }
}

function clamp(position: DragPosition, size: { width: number; height: number }): DragPosition {
  const maxRight = Math.max(0, window.innerWidth - size.width)
  const maxBottom = Math.max(0, window.innerHeight - size.height)
  return {
    right: Math.min(Math.max(position.right, 0), maxRight),
    bottom: Math.min(Math.max(position.bottom, 0), maxBottom),
  }
}

function clampIfChanged(
  current: DragPosition,
  size: { width: number; height: number },
): DragPosition {
  const next = clamp(current, size)
  if (next.right === current.right && next.bottom === current.bottom) {
    return current
  }
  return next
}

/**
 * Makes an element draggable via its `right`/`bottom` CSS offsets, persisting
 * the resulting position to localStorage and clamping it to stay within the
 * viewport. Returns the current position (or `null` to fall back to the
 * default CSS position), drag-handle props, and a one-shot `wasDragged()` that
 * consumes the drag flag (suppresses the trailing click after a drag without
 * blocking later keyboard activation).
 */
export function useDraggablePosition(
  storageKey: string,
  elementRef: React.RefObject<HTMLElement | null>,
) {
  const [position, setPosition] = useState<DragPosition | null>(() => readStoredPosition(storageKey))
  const draggingRef = useRef(false)
  const movedRef = useRef(false)
  const startRef = useRef({ pointerX: 0, pointerY: 0, right: 0, bottom: 0 })

  const reclamp = useCallback(() => {
    setPosition((current) => {
      if (!current || !elementRef.current) return current
      const rect = elementRef.current.getBoundingClientRect()
      return clampIfChanged(current, { width: rect.width, height: rect.height })
    })
  }, [elementRef])

  // Clamp whenever the element is in the DOM (including first paint after a
  // stored position restore, and when the panel mounts after the FAB opens).
  useLayoutEffect(() => {
    reclamp()
  })

  useEffect(() => {
    window.addEventListener('resize', reclamp)
    return () => window.removeEventListener('resize', reclamp)
  }, [reclamp])

  const onPointerDown = useCallback(
    (event: React.PointerEvent) => {
      if (event.button !== 0) return
      const element = elementRef.current
      if (!element) return
      const rect = element.getBoundingClientRect()
      const currentRight = window.innerWidth - rect.right
      const currentBottom = window.innerHeight - rect.bottom
      draggingRef.current = true
      movedRef.current = false
      startRef.current = {
        pointerX: event.clientX,
        pointerY: event.clientY,
        right: currentRight,
        bottom: currentBottom,
      }
      ;(event.target as HTMLElement).setPointerCapture?.(event.pointerId)
    },
    [elementRef],
  )

  const onPointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!draggingRef.current) return
      const element = elementRef.current
      if (!element) return
      const deltaX = event.clientX - startRef.current.pointerX
      const deltaY = event.clientY - startRef.current.pointerY
      if (!movedRef.current && Math.hypot(deltaX, deltaY) < DRAG_THRESHOLD_PX) {
        return
      }
      movedRef.current = true
      const rect = element.getBoundingClientRect()
      const next = clamp(
        {
          right: startRef.current.right - deltaX,
          bottom: startRef.current.bottom - deltaY,
        },
        { width: rect.width, height: rect.height },
      )
      setPosition(next)
    },
    [elementRef],
  )

  const onPointerUp = useCallback((event: React.PointerEvent) => {
    if (!draggingRef.current) return
    draggingRef.current = false
    ;(event.target as HTMLElement).releasePointerCapture?.(event.pointerId)
    if (movedRef.current) {
      setPosition((current) => {
        if (current) {
          try {
            window.localStorage.setItem(storageKey, JSON.stringify(current))
          } catch {
            // ignore storage failures (e.g. quota, privacy mode)
          }
        }
        return current
      })
    }
  }, [storageKey])

  const wasDragged = useCallback(() => {
    const dragged = movedRef.current
    if (dragged) {
      movedRef.current = false
    }
    return dragged
  }, [])

  return {
    position,
    dragHandleProps: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel: onPointerUp,
    },
    wasDragged,
  }
}
