import { describe, expect, it } from 'vitest'

import { folderIconDataUri, gitlabIconDataUri, sourceIconStyle } from './icons'

describe('source icons', () => {
  it('encodes gitlab and folder data URIs with color', () => {
    expect(gitlabIconDataUri('#4f46e5')).toContain('%234f46e5')
    expect(folderIconDataUri('#94a3b8')).toContain('%2394a3b8')
  })

  it('sourceIconStyle picks gitlab uri when active', () => {
    const style = sourceIconStyle('gitlab', true)
    expect(style.backgroundImage).toContain('256 256')
    expect(style.backgroundImage).toContain('%234f46e5')
  })
})
