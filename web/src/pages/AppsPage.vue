<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import type { AppView } from '../api/types';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';

const { client } = useSession();
const router = useRouter();

const {
  data: apps,
  error: appsError,
  loading: appsLoading,
  reload: reloadApps,
} = useAsync<AppView[]>(() => client.listApps());

const actionError = ref<string | null>(null);
const displayError = computed(() => actionError.value ?? appsError.value);

const columns = [
  { id: 'name', header: '名称' },
  { id: 'id', header: 'App Id' },
  { id: 'enabled', header: '状态' },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

const creating = ref(false);
const createName = ref('');

const renaming = ref<AppView | null>(null);
const renameName = ref('');

/** 选中项驱动的弹窗：关闭（Esc/遮罩/按钮）时清空选中 */
const renameOpen = computed({
  get: () => renaming.value !== null,
  set: (value) => {
    if (!value) renaming.value = null;
  },
});

watch(creating, (open) => {
  if (open) createName.value = '';
});

watch(renaming, (target) => {
  if (target) renameName.value = target.name;
});

async function run(action: () => Promise<unknown>): Promise<boolean> {
  actionError.value = null;
  try {
    await action();
    reloadApps();
    return true;
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

async function createApp(): Promise<void> {
  if (await run(() => client.createApp({ name: createName.value.trim() }))) creating.value = false;
}

async function renameApp(): Promise<void> {
  const target = renaming.value;
  if (!target) return;
  if (await run(() => client.updateApp(target.id, { name: renameName.value.trim() }))) {
    renaming.value = null;
  }
}

async function removeApp(app: AppView): Promise<void> {
  if (await askConfirm(`确认删除应用「${app.name}」？删除后不可恢复。`)) {
    await run(() => client.deleteApp(app.id));
  }
}

const menuItems = (app: AppView) => [
  [{ label: '改名', icon: 'i-lucide-pencil', onSelect: () => (renaming.value = app) }],
  [
    {
      label: app.enabled ? '禁用' : '启用',
      icon: app.enabled ? 'i-lucide-circle-slash' : 'i-lucide-circle-check',
      onSelect: () => void run(() => client.updateApp(app.id, { enabled: !app.enabled })),
    },
    {
      label: '删除',
      icon: 'i-lucide-trash',
      color: 'error' as const,
      onSelect: () => void removeApp(app),
    },
  ],
];
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-4">
        <div>
          <h2 class="font-semibold text-highlighted">应用</h2>
          <p class="text-sm text-muted">全部应用记录（含已禁用）</p>
        </div>
        <div class="flex shrink-0 gap-2">
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            icon="i-lucide-refresh-cw"
            label="刷新"
            :loading="appsLoading"
            @click="reloadApps"
          />
          <UButton size="sm" icon="i-lucide-plus" label="新建应用" @click="creating = true" />
        </div>
      </div>
    </template>

    <UAlert
      v-if="displayError"
      class="mb-4"
      color="error"
      variant="subtle"
      icon="i-lucide-triangle-alert"
      :description="displayError"
    />

    <UTable :data="apps ?? []" :columns="columns" :loading="appsLoading">
      <template #name-cell="{ row }">
        <span class="font-medium">{{ row.original.name }}</span>
      </template>

      <template #id-cell="{ row }">
        <span class="font-mono text-xs text-muted">{{ row.original.id }}</span>
      </template>

      <template #enabled-cell="{ row }">
        <UBadge
          :color="row.original.enabled ? 'success' : 'neutral'"
          variant="subtle"
          size="sm"
          :label="row.original.enabled ? '启用' : '禁用'"
        />
      </template>

      <template #actions-cell="{ row }">
        <div class="flex items-center justify-end gap-1">
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            label="详情"
            @click="router.push({ name: 'app-detail', params: { id: row.original.id } })"
          />
          <UDropdownMenu :items="menuItems(row.original)">
            <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-ellipsis" />
          </UDropdownMenu>
        </div>
      </template>

      <template #empty>
        <UEmpty
          icon="i-lucide-boxes"
          title="暂无应用"
          description="点击右上角「新建应用」创建第一个应用"
        />
      </template>
    </UTable>

    <UModal v-model:open="creating" title="新建应用" :ui="{ footer: 'justify-end' }">
      <template #body>
        <UFormField label="应用名" required>
          <UInput v-model="createName" class="w-full" autofocus />
        </UFormField>
        <p class="mt-2 text-xs text-dimmed">App Id 可留空，由服务端生成 UUID v4</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="creating = false" />
        <UButton label="创建" :disabled="createName.trim() === ''" @click="createApp" />
      </template>
    </UModal>

    <UModal
      v-model:open="renameOpen"
      :title="renaming ? `改名：${renaming.name}` : '改名'"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <UFormField label="新名称">
          <UInput v-model="renameName" class="w-full" autofocus />
        </UFormField>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="renaming = null" />
        <UButton label="保存" :disabled="renameName.trim() === ''" @click="renameApp" />
      </template>
    </UModal>
  </UCard>
</template>
