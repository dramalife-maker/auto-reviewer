import { resolve } from 'node:path'

import { defineConfig, loadEnv } from 'vite'

import { normalizeBasePath } from './base'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const rootEnv = loadEnv(mode, resolve(__dirname, '..'), '')
  const base = normalizeBasePath(env.VITE_BASE_PATH)
  const backendPort = rootEnv.PORT || '8080'
  const backendOrigin = `http://127.0.0.1:${backendPort}`

  return {
    base,
    server: {
      proxy: {
        '/health': backendOrigin,
        '/api': backendOrigin,
      },
    },
  }
})
