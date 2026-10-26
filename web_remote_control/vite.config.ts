import { fileURLToPath, URL } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

// 开发时把 API 和 WebSocket 代理到本机网关，保持与部署时同源，无需 --allow-origin。
const gateway = process.env.ROBOT_REMOTE_URL ?? 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  server: {
    host: true,
    proxy: {
      '/api': gateway,
      '/healthz': gateway,
      '/ws': { target: gateway, ws: true },
    },
  },
})
