import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: '127.0.0.1',
    port: 5173,
    // 开发时直连本地服务；生产靠构建期注入的 VITE_BASE_API
    proxy: {
      '/api': {
        target: process.env.VITE_BASE_API ?? 'http://127.0.0.1:8081',
        changeOrigin: true,
      },
    },
  },
});
