import './assets/css/main.css';
import { createApp } from 'vue';
import ui from '@nuxt/ui/vue-plugin';
import App from './App.vue';
import { router } from './router';

// 深色模式由 @nuxt/ui/vue-plugin 内置的 color-mode 插件跟随系统偏好（html 上切 .dark）
const app = createApp(App);
app.use(router);
app.use(ui);
app.mount('#app');
