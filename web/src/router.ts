import { createRouter, createWebHistory } from 'vue-router';
import AnnouncesPage from './pages/AnnouncesPage.vue';
import AppDetailPage from './pages/AppDetailPage.vue';
import AppsPage from './pages/AppsPage.vue';
import PoolsPage from './pages/PoolsPage.vue';
import ResourcesPage from './pages/ResourcesPage.vue';

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/apps' },
    { path: '/apps', name: 'apps', component: AppsPage },
    { path: '/apps/:id', name: 'app-detail', component: AppDetailPage, props: true },
    { path: '/pools', name: 'pools', component: PoolsPage },
    { path: '/resources', name: 'resources', component: ResourcesPage },
    { path: '/announces', name: 'announces', component: AnnouncesPage },
    { path: '/:pathMatch(.*)*', redirect: '/apps' },
  ],
});
