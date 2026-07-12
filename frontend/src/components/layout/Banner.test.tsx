import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { Banner } from './Banner.tsx'
import { BannerProvider, useBanner } from '../../context/BannerContext.tsx'

function Trigger() {
  const banner = useBanner()
  return (
    <button type="button" onClick={() => banner.show('失敗了', true)}>
      觸發
    </button>
  )
}

describe('Banner', () => {
  it('shows error banner and dismisses', async () => {
    const user = userEvent.setup()
    render(
      <BannerProvider>
        <Trigger />
        <Banner />
      </BannerProvider>,
    )
    await user.click(screen.getByRole('button', { name: '觸發' }))
    expect(screen.getByText('失敗了')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '關閉提示' }))
    expect(screen.queryByText('失敗了')).not.toBeInTheDocument()
  })
})
