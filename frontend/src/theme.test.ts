import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

import { DESIGN_TOKENS } from './lib/tokens'

const root = dirname(fileURLToPath(import.meta.url))

describe('design tokens', () => {
  it('defines indigo primary and MR violet in CSS theme', () => {
    const css = readFileSync(resolve(root, 'index.css'), 'utf8')
    expect(css).toContain('@theme')
    expect(css).toContain(DESIGN_TOKENS.primary)
    expect(css).toContain(DESIGN_TOKENS.mr)
  })
})
