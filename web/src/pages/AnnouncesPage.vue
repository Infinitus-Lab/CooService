<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type { AnnounceView, AppView } from '../api/types';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';
import { fromIso, period, statusBadge, toIso, validatePeriod } from '../lib/announce';
import HashChip from '../components/HashChip.vue';

/** 公告台账：全局内容，app 通过引用关联（多 app 可共享同一条）。 */
const { client } = useSession();

const { data: apps } = useAsync<AppView[]>(() => client.listApps());

const filter = ref('');

const {
  data: announces,
  error: announcesError,
  loading: announcesLoading,
  reload: reloadAnnounces,
} = useAsync<AnnounceView[]>(
  () => client.listAnnounces(filter.value || undefined),
  () => [filter.value],
);

const actionError = ref<string | null>(null);
const displayError = computed(() => actionError.value ?? announcesError.value);

const columns = [
  { id: 'title', header: '标题' },
  { id: 'content', header: '内容' },
  { id: 'period', header: '有效期' },
  { id: 'status', header: '状态' },
  { id: 'ref_apps', header: '引用应用' },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

const filterItems = computed(() => [
  { label: '全部公告', value: '' },
  ...(apps.value ?? []).map((app) => ({ label: app.name, value: app.id })),
]);

async function run(action: () => Promise<unknown>): Promise<boolean> {
  actionError.value = null;
  try {
    await action();
    reloadAnnounces();
    return true;
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

interface AnnounceForm {
  title: string;
  content: string;
  starts_at: string;
  expires_at: string;
}

function emptyForm(): AnnounceForm {
  return { title: '', content: '', starts_at: '', expires_at: '' };
}

// ---------- 新建 ----------

const creating = ref(false);
const createForm = reactive<AnnounceForm>(emptyForm());
const createError = ref<string | null>(null);

watch(creating, (open) => {
  if (open) {
    Object.assign(createForm, emptyForm());
    createError.value = null;
  }
});

async function submitCreate(): Promise<void> {
  if (!createForm.title.trim() || !createForm.content.trim()) {
    createError.value = '标题、内容必填';
    return;
  }
  const problem = validatePeriod(createForm.starts_at, createForm.expires_at);
  if (problem) {
    createError.value = problem;
    return;
  }
  createError.value = null;
  const ok = await run(() =>
    client.createAnnounce({
      title: createForm.title.trim(),
      content: createForm.content.trim(),
      starts_at: toIso(createForm.starts_at),
      expires_at: toIso(createForm.expires_at),
    }),
  );
  if (ok) creating.value = false;
}

// ---------- 编辑 ----------

const editing = ref<AnnounceView | null>(null);
const editForm = reactive<AnnounceForm>(emptyForm());
const editError = ref<string | null>(null);

watch(editing, (target) => {
  if (target) {
    editForm.title = target.title;
    editForm.content = target.content;
    editForm.starts_at = fromIso(target.starts_at);
    editForm.expires_at = fromIso(target.expires_at);
    editError.value = null;
  }
});

const editOpen = computed({
  get: () => editing.value !== null,
  set: (value) => {
    if (!value) editing.value = null;
  },
});

async function submitEdit(): Promise<void> {
  const target = editing.value;
  if (!target) return;
  if (!editForm.title.trim() || !editForm.content.trim()) {
    editError.value = '标题、内容必填';
    return;
  }
  const problem = validatePeriod(editForm.starts_at, editForm.expires_at);
  if (problem) {
    editError.value = problem;
    return;
  }
  editError.value = null;
  // PATCH 全量语义：四个字段一次给全
  const ok = await run(() =>
    client.updateAnnounce(target.guid, {
      title: editForm.title.trim(),
      content: editForm.content.trim(),
      starts_at: toIso(editForm.starts_at),
      expires_at: toIso(editForm.expires_at),
    }),
  );
  if (ok) editing.value = null;
}

async function removeAnnounce(announce: AnnounceView): Promise<void> {
  if (await askConfirm(`确认删除公告「${announce.title}」？`)) {
    await run(() => client.deleteAnnounce(announce.guid));
  }
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="font-semibold text-highlighted">公告</h2>
          <p class="text-sm text-muted">全局内容，由应用引用；仅在有效期内对客户端展示</p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <USelect v-model="filter" :items="filterItems" size="sm" class="w-44" />
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            icon="i-lucide-refresh-cw"
            label="刷新"
            :loading="announcesLoading"
            @click="reloadAnnounces"
          />
          <UButton size="sm" icon="i-lucide-plus" label="发布公告" @click="creating = true" />
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

    <UTable :data="announces ?? []" :columns="columns" :loading="announcesLoading">
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

      <template #ref_apps-cell="{ row }">
        <span class="text-sm">
          {{ row.original.ref_apps.length > 0 ? row.original.ref_apps.join('、') : '—' }}
        </span>
      </template>

      <template #actions-cell="{ row }">
        <div class="flex items-center justify-end gap-1">
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            label="编辑"
            @click="editing = row.original"
          />
          <UButton
            size="xs"
            color="error"
            variant="ghost"
            label="删除"
            @click="removeAnnounce(row.original)"
          />
        </div>
      </template>

      <template #empty>
        <UEmpty
          icon="i-lucide-megaphone"
          title="暂无公告"
          description="点击右上角「发布公告」创建第一条公告"
        />
      </template>
    </UTable>

    <UModal v-model:open="creating" title="发布公告" :ui="{ footer: 'justify-end' }">
      <template #body>
        <UFormField label="标题" required>
          <UInput v-model="createForm.title" class="w-full" />
        </UFormField>
        <UFormField label="内容" required class="mt-4">
          <UTextarea v-model="createForm.content" :rows="4" class="w-full" />
        </UFormField>
        <div class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="展示起始时间" hint="留空立即生效">
            <UInput v-model="createForm.starts_at" type="datetime-local" class="w-full font-mono" />
          </UFormField>
          <UFormField label="展示截止时间" hint="留空不限">
            <UInput v-model="createForm.expires_at" type="datetime-local" class="w-full font-mono" />
          </UFormField>
        </div>
        <p class="mt-3 text-xs text-dimmed">发布后需在应用详情页关联，客户端方可查看</p>
        <p v-if="createError" class="mt-2 text-xs text-error">{{ createError }}</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="creating = false" />
        <UButton label="发布" @click="submitCreate" />
      </template>
    </UModal>

    <UModal
      v-model:open="editOpen"
      :title="editing ? `编辑公告：${editing.title}` : '编辑公告'"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <p v-if="editing" class="mb-3 text-xs text-muted">
          公告 Id：<HashChip :hash="editing.guid" :head="10" />
        </p>
        <UFormField label="标题" required>
          <UInput v-model="editForm.title" class="w-full" />
        </UFormField>
        <UFormField label="内容" required class="mt-4">
          <UTextarea v-model="editForm.content" :rows="4" class="w-full" />
        </UFormField>
        <div class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="展示起始时间" hint="清空立即生效">
            <UInput v-model="editForm.starts_at" type="datetime-local" class="w-full font-mono" />
          </UFormField>
          <UFormField label="展示截止时间" hint="清空不限">
            <UInput v-model="editForm.expires_at" type="datetime-local" class="w-full font-mono" />
          </UFormField>
        </div>
        <p class="mt-3 text-xs text-dimmed">
          已引用应用：{{ editing && editing.ref_apps.length > 0 ? editing.ref_apps.join('、') : '无' }}
        </p>
        <p v-if="editError" class="mt-2 text-xs text-error">{{ editError }}</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="editing = null" />
        <UButton label="保存" @click="submitEdit" />
      </template>
    </UModal>
  </UCard>
</template>
