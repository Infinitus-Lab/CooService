<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import type { AnnounceView, AppResourceView, AppView, ResourceView } from '../api/types';
import HashChip from '../components/HashChip.vue';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';
import { formatBytes } from '../lib/format';
import { period, statusBadge } from '../lib/announce';
import ChannelsPage from './ChannelsPage.vue';

type View = 'overview' | 'channels' | 'resources' | 'announces';

/** App 详情：左侧栏导航，四个视图都按当前 app 关联/过滤。 */
const props = defineProps<{ id: string }>();

const { client } = useSession();
const router = useRouter();

// 直达/刷新时按 id 从应用列表解析出当前 app
const { data: apps, loading: appsLoading } = useAsync<AppView[]>(() => client.listApps());
const app = computed(() => (apps.value ?? []).find((item) => item.id === props.id) ?? null);
const notFound = computed(() => !appsLoading.value && apps.value !== null && app.value === null);

const view = ref<View>('overview');
const actionError = ref<string | null>(null);

const {
  data: channels,
  loading: channelsLoading,
} = useAsync(() => client.listChannelsAdmin(props.id), () => [props.id]);

const {
  data: linked,
  error: linkedError,
  loading: linkedLoading,
  reload: reloadLinked,
} = useAsync<AppResourceView[]>(() => client.listAppResources(props.id), () => [props.id]);

const {
  data: announces,
  error: announcesError,
  loading: announcesLoading,
  reload: reloadAnnounces,
} = useAsync<AnnounceView[]>(() => client.listAppAnnounces(props.id), () => [props.id]);

const displayError = computed(() => actionError.value ?? linkedError.value ?? announcesError.value);

async function run(action: () => Promise<unknown>): Promise<boolean> {
  actionError.value = null;
  try {
    await action();
    reloadLinked();
    reloadAnnounces();
    return true;
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

const nav = computed<{ value: View; label: string; count: string }[]>(() => [
  { value: 'overview', label: '概览', count: '' },
  { value: 'channels', label: '发版通道', count: String(channels.value?.length ?? 0) },
  { value: 'resources', label: '资源', count: String(linked.value?.length ?? 0) },
  { value: 'announces', label: '公告', count: String(announces.value?.length ?? 0) },
]);

const totalLinkedSize = computed(() => (linked.value ?? []).reduce((sum, r) => sum + r.size, 0));

const linkedColumns = [
  { id: 'sha256', header: '指纹' },
  { id: 'size', header: '大小', meta: { class: { th: 'text-right', td: 'text-right' } } },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

const announceColumns = [
  { id: 'title', header: '标题' },
  { id: 'content', header: '内容' },
  { id: 'period', header: '有效期' },
  { id: 'status', header: '状态' },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

// ---------- 关联资源 ----------

const linkingResource = ref(false);
const resourceQuery = ref('');
const pickedResource = ref('');

const {
  data: allResources,
  error: allResourcesError,
  loading: allResourcesLoading,
} = useAsync<ResourceView[]>(
  () => (linkingResource.value ? client.listResources() : Promise.resolve([])),
  () => [linkingResource.value],
);

watch(linkingResource, (open) => {
  if (open) {
    resourceQuery.value = '';
    pickedResource.value = '';
  }
});

const resourceCandidates = computed(() =>
  (allResources.value ?? []).filter(
    (resource) =>
      resourceQuery.value.trim() === '' ||
      resource.sha256.startsWith(resourceQuery.value.trim().toLowerCase()),
  ),
);

async function linkResource(): Promise<void> {
  if (!pickedResource.value) return;
  if (await run(() => client.linkAppResource(props.id, pickedResource.value))) {
    linkingResource.value = false;
  }
}

async function unlinkResource(resource: AppResourceView): Promise<void> {
  if (await askConfirm('确认解除关联？资源本身不会被删除。')) {
    await run(() => client.unlinkAppResource(props.id, resource.sha256));
  }
}

// ---------- 关联公告 ----------

const linkingAnnounce = ref(false);
const selectedAnnounce = ref('');

const {
  data: allAnnounces,
  error: allAnnouncesError,
  loading: allAnnouncesLoading,
} = useAsync<AnnounceView[]>(
  () => (linkingAnnounce.value ? client.listAnnounces() : Promise.resolve([])),
  () => [linkingAnnounce.value],
);

watch(linkingAnnounce, (open) => {
  if (open) selectedAnnounce.value = '';
});

const linkedGuids = computed(() => new Set((announces.value ?? []).map((a) => a.guid)));

const announceCandidates = computed(() =>
  (allAnnounces.value ?? []).filter((a) => !linkedGuids.value.has(a.guid)),
);

const announceOptions = computed(() =>
  announceCandidates.value.map((a) => ({
    label: `${a.title} · ${a.guid.slice(0, 8)} · ${statusBadge(a.status).label}`,
    value: a.guid,
  })),
);

async function linkAnnounce(): Promise<void> {
  if (!selectedAnnounce.value) return;
  if (await run(() => client.linkAppAnnounce(props.id, selectedAnnounce.value))) {
    linkingAnnounce.value = false;
  }
}

async function unlinkAnnounce(announce: AnnounceView): Promise<void> {
  if (await askConfirm('确认解除关联？公告本身不会被删除。')) {
    await run(() => client.unlinkAppAnnounce(props.id, announce.guid));
  }
}
</script>

<template>
  <div class="space-y-4">
    <UBreadcrumb
      :items="[
        { label: '应用', icon: 'i-lucide-boxes', to: '/apps' },
        { label: app?.name ?? id },
      ]"
    />

    <UAlert
      v-if="notFound"
      color="warning"
      variant="subtle"
      icon="i-lucide-search-x"
      title="应用不存在"
      description="该 App Id 在当前服务上找不到，可能已被删除。"
      :actions="[
        {
          label: '返回应用列表',
          color: 'neutral',
          variant: 'outline',
          onClick: () => router.push('/apps'),
        },
      ]"
    />

    <div v-else-if="app" class="grid gap-4 lg:grid-cols-[13rem_1fr]">
      <aside>
        <UCard :ui="{ body: 'gap-3' }">
          <div class="border-b border-default pb-3">
            <div class="flex items-center gap-2">
              <span class="text-lg font-bold text-highlighted">{{ app.name }}</span>
              <UBadge
                :color="app.enabled ? 'success' : 'neutral'"
                variant="subtle"
                size="sm"
                :label="app.enabled ? '启用' : '禁用'"
              />
            </div>
            <div class="mt-1">
              <HashChip :hash="app.id" :head="10" />
            </div>
          </div>

          <nav class="flex flex-col gap-1">
            <UButton
              v-for="item in nav"
              :key="item.value"
              size="sm"
              block
              :variant="view === item.value ? 'soft' : 'ghost'"
              :color="view === item.value ? 'primary' : 'neutral'"
              class="justify-between"
              @click="view = item.value"
            >
              <span>{{ item.label }}</span>
              <span v-if="item.count !== ''" class="text-xs text-dimmed">{{ item.count }}</span>
            </UButton>
          </nav>
        </UCard>
      </aside>

      <main class="min-w-0">
        <UAlert
          v-if="displayError"
          class="mb-4"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="displayError"
        />

        <!-- 概览 -->
        <UCard v-if="view === 'overview'">
          <template #header>
            <h2 class="font-semibold text-highlighted">概览</h2>
          </template>
          <div class="grid gap-4 sm:grid-cols-3">
            <div class="rounded-lg border border-default p-4">
              <p class="text-xs tracking-widest text-dimmed uppercase">发版通道</p>
              <p class="mt-1 font-mono text-2xl font-bold text-highlighted">
                {{ channelsLoading ? '—' : (channels?.length ?? '—') }}
              </p>
              <p class="mt-1 text-xs text-muted">
                {{
                  channels && channels.length > 0
                    ? `默认：${channels.find((c) => c.is_default)?.tag_name ?? '无'}`
                    : '尚未创建'
                }}
              </p>
            </div>
            <div class="rounded-lg border border-default p-4">
              <p class="text-xs tracking-widest text-dimmed uppercase">关联资源</p>
              <p class="mt-1 font-mono text-2xl font-bold text-highlighted">
                {{ linkedLoading ? '—' : (linked?.length ?? '—') }}
              </p>
              <p class="mt-1 text-xs text-muted">
                {{
                  linked && linked.length > 0
                    ? `共 ${formatBytes(totalLinkedSize)}`
                    : '尚未关联'
                }}
              </p>
            </div>
            <div class="rounded-lg border border-default p-4">
              <p class="text-xs tracking-widest text-dimmed uppercase">公告</p>
              <p class="mt-1 font-mono text-2xl font-bold text-highlighted">
                {{ announcesLoading ? '—' : (announces?.length ?? '—') }}
              </p>
              <p class="mt-1 text-xs text-muted">
                {{
                  announces && announces.length > 0 ? `最新：${announces[0]!.title}` : '尚未引用'
                }}
              </p>
            </div>
          </div>
          <p class="mt-4 text-sm text-muted">
            可通过左侧栏查看本应用的发版通道、资源与公告；资源与公告均为全局内容，关联仅为引用。
          </p>
        </UCard>

        <!-- 发版通道 -->
        <ChannelsPage v-else-if="view === 'channels'" :app-id="app.id" />

        <!-- 关联资源 -->
        <UCard v-else-if="view === 'resources'">
          <template #header>
            <div class="flex items-center justify-between gap-4">
              <div>
                <h2 class="font-semibold text-highlighted">关联资源</h2>
                <p class="text-sm text-muted">引用全局资源库内容；解绑不影响资源本身</p>
              </div>
              <UButton size="sm" icon="i-lucide-link" label="关联资源" @click="linkingResource = true" />
            </div>
          </template>

          <UTable :data="linked ?? []" :columns="linkedColumns" :loading="linkedLoading">
            <template #sha256-cell="{ row }">
              <HashChip :hash="row.original.sha256" />
            </template>
            <template #size-cell="{ row }">
              <span class="tabular-nums">{{ formatBytes(row.original.size) }}</span>
            </template>
            <template #actions-cell="{ row }">
              <UButton
                size="xs"
                color="error"
                variant="ghost"
                label="解绑"
                @click="unlinkResource(row.original)"
              />
            </template>
            <template #empty>
              <UEmpty
                icon="i-lucide-package-open"
                title="尚未关联任何资源"
                description="请先在「资源」页上传，再执行关联"
              />
            </template>
          </UTable>
        </UCard>

        <!-- 关联公告 -->
        <UCard v-else-if="view === 'announces'">
          <template #header>
            <div class="flex items-center justify-between gap-4">
              <div>
                <h2 class="font-semibold text-highlighted">公告</h2>
                <p class="text-sm text-muted">引用全局公告库内容；多条应用可共享同一公告</p>
              </div>
              <UButton size="sm" icon="i-lucide-link" label="关联公告" @click="linkingAnnounce = true" />
            </div>
          </template>

          <UTable :data="announces ?? []" :columns="announceColumns" :loading="announcesLoading">
            <template #title-cell="{ row }">
              <span class="font-medium">{{ row.original.title }}</span>
            </template>
            <template #content-cell="{ row }">
              <span class="block max-w-60 truncate text-sm text-muted">{{ row.original.content }}</span>
            </template>
            <template #period-cell="{ row }">
              <span class="text-xs text-muted">
                {{ period(row.original.starts_at, row.original.expires_at) }}
              </span>
            </template>
            <template #status-cell="{ row }">
              <UBadge
                :color="statusBadge(row.original.status).color"
                variant="subtle"
                size="sm"
                :label="statusBadge(row.original.status).label"
              />
            </template>
            <template #actions-cell="{ row }">
              <UButton
                size="xs"
                color="error"
                variant="ghost"
                label="解绑"
                @click="unlinkAnnounce(row.original)"
              />
            </template>
            <template #empty>
              <UEmpty
                icon="i-lucide-megaphone"
                title="尚未引用任何公告"
                description="请先在「公告」页发布，再执行关联"
              />
            </template>
          </UTable>
        </UCard>
      </main>
    </div>

    <!-- 选资源关联 -->
    <UModal v-model:open="linkingResource" title="选择资源" :ui="{ footer: 'justify-end' }">
      <template #body>
        <UFormField label="搜索资源（sha256 前缀，不输入显示全部）">
          <UInput
            v-model="resourceQuery"
            class="w-full font-mono"
            placeholder="4b540700c4f1…"
            autofocus
            @update:model-value="pickedResource = ''"
          />
        </UFormField>

        <div v-if="allResourcesLoading" class="flex justify-center py-10">
          <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="allResourcesError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="allResourcesError"
        />
        <UEmpty
          v-else-if="(allResources ?? []).length === 0"
          icon="i-lucide-package-open"
          title="暂无资源"
          description="请先在「资源」页上传"
        />
        <div v-else class="mt-3 max-h-64 space-y-1 overflow-y-auto rounded-md border border-default p-2">
          <p v-if="resourceCandidates.length === 0" class="px-2 py-3 text-center text-sm text-muted">
            未找到匹配的资源
          </p>
          <div
            v-for="resource in resourceCandidates"
            :key="resource.sha256"
            role="radio"
            tabindex="0"
            :aria-checked="pickedResource === resource.sha256"
            class="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 transition-colors"
            :class="pickedResource === resource.sha256 ? 'bg-elevated' : 'hover:bg-elevated/50'"
            @click="pickedResource = resource.sha256"
            @keydown.enter="pickedResource = resource.sha256"
          >
            <UIcon
              :name="pickedResource === resource.sha256 ? 'i-lucide-circle-dot' : 'i-lucide-circle'"
              class="size-4 shrink-0 text-dimmed"
            />
            <HashChip :hash="resource.sha256" :head="10" />
            <span class="ms-auto text-xs tabular-nums text-muted">
              {{ formatBytes(resource.size) }} · {{ resource.pools.join('/') }}
            </span>
          </div>
        </div>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="linkingResource = false" />
        <UButton label="关联" :disabled="pickedResource === ''" @click="linkResource" />
      </template>
    </UModal>

    <!-- 选公告关联 -->
    <UModal v-model:open="linkingAnnounce" title="关联公告" :ui="{ footer: 'justify-end' }">
      <template #body>
        <div v-if="allAnnouncesLoading" class="flex justify-center py-10">
          <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="allAnnouncesError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="allAnnouncesError"
        />
        <p v-else-if="announceCandidates.length === 0" class="text-sm text-muted">
          暂无可用公告（已全部关联或公告库为空）
        </p>
        <USelect
          v-else
          v-model="selectedAnnounce"
          :items="announceOptions"
          class="w-full"
          placeholder="选择公告"
        />
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="linkingAnnounce = false" />
        <UButton label="关联" :disabled="selectedAnnounce === ''" @click="linkAnnounce" />
      </template>
    </UModal>
  </div>
</template>
