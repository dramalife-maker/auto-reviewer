import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { Toast } from './Toast.tsx'
import { ToastProvider, useToast } from '../../context/ToastContext.tsx'

function Trigger({ error = false }: { error?: boolean }) {
  const toast = useToast()
  return (
    <button type="button" onClick={() => toast.show(error ? '失敗了' : '已儲存', error)}>
      觸發
    </button>
  )
}

describe('Toast', () => {
  afterEach(() => {
    cleanup()
  })

  it('shows error toast and dismisses', () => {
    render(
      <ToastProvider>
        <Trigger error />
        <Toast />
      </ToastProvider>,
    )
    fireEvent.click(screen.getByRole('button', { name: '觸發' }))
    expect(screen.getByText('失敗了')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '關閉提示' }))
    expect(screen.queryByText('失敗了')).not.toBeInTheDocument()
  })

  it('auto-dismisses success toast', () => {
    vi.useFakeTimers()
    render(
      <ToastProvider>
        <Trigger />
        <Toast />
      </ToastProvider>,
    )
    fireEvent.click(screen.getByRole('button', { name: '觸發' }))
    expect(screen.getByText('已儲存')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(4000)
    })
    expect(screen.queryByText('已儲存')).not.toBeInTheDocument()
    vi.useRealTimers()
  })
})
