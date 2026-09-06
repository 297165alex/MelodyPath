import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', '')
  const apiTarget = env.VITE_API_PROXY_TARGET || 'http://127.0.0.1:3000'

  return {
    plugins: [react()],
    server: {
      host: '127.0.0.1',
      port: 3001,
      proxy: {
        '/api': apiTarget,
        '/health': apiTarget,
      },
    },
  }
})