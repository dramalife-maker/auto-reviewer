import { defineConfig, loadEnv } from 'vite'

import { normalizeBasePath } from './base'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const base = normalizeBasePath(env.VITE_BASE_PATH)

  return {
    base,
    server: {
      proxy: {
        '/health': 'http://127.0.0.1:8080',
        '/api': 'http://127.0.0.1:8080',
      },
    },
  }
})
