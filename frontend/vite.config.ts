import { resolve } from 'node:path'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
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
    plugins: [react(), tailwindcss()],
    server: {
      proxy: {
        '/health': backendOrigin,
        '/api': backendOrigin,
      },
    },
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test/setup.ts'],
    },
  }
})
