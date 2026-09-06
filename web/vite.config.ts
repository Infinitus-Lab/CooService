import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: '127.0.0.1',
    port: 5173,
    // 无 /api 代理：API 地址由 WebUI 内「设置」框配置（留空 = 同源），浏览器直连（CORS）
  },
});
