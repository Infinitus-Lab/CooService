<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import ApiSetupDialog from './components/ApiSetupDialog.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import { useSession } from './composables/useSession';

const { adminKey, baseApi, gateOpen, gateFocus, epoch, disconnect } = useSession();
const router = useRouter();

const nav = [
  { label: '应用', icon: 'i-lucide-boxes', to: '/apps' },
  {
    label: '资源管理',
    icon: 'i-lucide-database',
    children: [
      { label: '资源池', to: '/pools' },
      { label: '资源', to: '/resources' },
    ],
  },
  { label: '公告', icon: 'i-lucide-megaphone', to: '/announces' },
];

const keyLabel = computed(() =>
  adminKey.value ? `密钥 ${adminKey.value.slice(0, 8)}…` : '未配置密钥',
);

/** 手动打开设置框：已配置地址则聚焦密钥，否则聚焦地址 */
function openSettings(): void {
  gateFocus.value = baseApi.value ? 'key' : 'base';
  gateOpen.value = true;
}

function onDisconnect(): void {
  disconnect();
  void router.push('/apps');
}
</script>

<template>
  <UApp>
    <div class="flex min-h-screen flex-col">
      <header class="border-b border-default bg-default px-4">
        <div class="mx-auto flex w-full max-w-6xl items-center gap-4 py-2">
          <RouterLink to="/apps" class="shrink-0 font-mono text-sm font-bold tracking-[0.22em]">
            COO<span class="text-primary">SERVICE</span>
            <span class="text-primary"> ●</span>
          </RouterLink>

          <UNavigationMenu
            :items="nav"
            orientation="horizontal"
            highlight
            class="min-w-0 flex-1"
          />

          <div class="flex shrink-0 items-center gap-2">
            <span class="hidden font-mono text-xs text-dimmed sm:inline">
              {{ baseApi || '同源' }}
            </span>
            <UBadge
              :color="adminKey ? 'success' : 'warning'"
              variant="subtle"
              :label="keyLabel"
              :title="adminKey ? '管理密钥已配置' : '未配置：触发管理请求时将要求输入密钥'"
            />
            <UButton size="sm" variant="ghost" color="neutral" label="设置" @click="openSettings" />
            <UButton size="sm" variant="ghost" color="neutral" label="断开" @click="onDisconnect" />
          </div>
        </div>
      </header>

      <main class="mx-auto w-full max-w-6xl flex-1 px-4 py-6">
        <RouterView :key="epoch" />
      </main>

      <footer class="p-4 text-center text-xs text-dimmed">
        CooService 管理台 · API 地址与管理密钥仅保存在当前标签页（sessionStorage），地址不可达或接口返回
        401 时将要求重新配置
      </footer>

      <ApiSetupDialog v-if="gateOpen" :focus="gateFocus" />
      <ConfirmDialog />
    </div>
  </UApp>
</template>
