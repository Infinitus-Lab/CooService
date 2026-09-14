<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type { AdminChannelView } from '../api/types';
import ChannelDetail from '../components/ChannelDetail.vue';
import HashChip from '../components/HashChip.vue';
import ResourcePicker from '../components/ResourcePicker.vue';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';
import { formatBytes } from '../lib/format';

/** 发版通道：应用内管理。行展开看版本记录与差分。 */
const props = defineProps<{ appId: string }>();

const { client } = useSession();

const {
  data: channels,
  error: channelsError,
  loading: channelsLoading,
  reload: reloadChannels,
} = useAsync<AdminChannelView[]>(
  () => client.listChannelsAdmin(props.appId),
  () => [props.appId],
);

const actionError = ref<string | null>(null);
const displayError = computed(() => actionError.value ?? channelsError.value);

const columns = [
  { id: 'tag_name', header: 'tag' },
  { id: 'latest_version', header: '最新版本' },
  { id: 'raw_sha256', header: '完整包指纹' },
  { id: 'raw_size', header: '大小', meta: { class: { th: 'text-right', td: 'text-right' } } },
  { id: 'is_default', header: '默认' },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

// ---------- 行展开（TanStack expanded 以行下标为 key） ----------

const expanded = ref<Record<string, boolean>>({});

function isExpanded(index: number): boolean {
  return !!expanded.value[String(index)];
}

function toggleExpand(index: number): void {
  const key = String(index);
  expanded.value = expanded.value[key] ? {} : { [key]: true };
}

async function run(action: () => Promise<unknown>): Promise<boolean> {
  actionError.value = null;
  try {
    await action();
    reloadChannels();
    return true;
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

// ---------- 新建 ----------

const creating = ref(false);
const createError = ref<string | null>(null);
const createForm = reactive({
  tag_name: '',
  latest_version: '',
  latest_sha256: null as string | null,
  is_default: false,
});

watch(creating, (open) => {
  if (open) {
    createForm.tag_name = '';
    createForm.latest_version = '';
    createForm.latest_sha256 = null;
    createForm.is_default = false;
    createError.value = null;
  }
});

async function submitCreate(): Promise<void> {
  if (!createForm.tag_name.trim() || !createForm.latest_version.trim()) {
    createError.value = 'tag、版本必填';
    return;
  }
  if (!createForm.latest_sha256) {
    createError.value = '请选择完整包资源';
    return;
  }
  createError.value = null;
  const ok = await run(() =>
    client.createChannel({
      app_id: props.appId,
      tag_name: createForm.tag_name.trim(),
      latest_version: createForm.latest_version.trim(),
      latest_sha256: createForm.latest_sha256!.toLowerCase(),
      is_default: createForm.is_default,
    }),
  );
  if (ok) creating.value = false;
}

// ---------- 编辑 ----------

const editing = ref<AdminChannelView | null>(null);
const editError = ref<string | null>(null);
const editForm = reactive({
  tag: '',
  version: '',
  sha: null as string | null,
  isDefault: false,
});

watch(editing, (channel) => {
  if (channel) {
    editForm.tag = channel.tag_name;
    editForm.version = channel.latest_version;
    editForm.sha = channel.raw_sha256;
    editForm.isDefault = channel.is_default;
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

  // 只提交变化过的字段
  const body: Record<string, unknown> = {};
  if (editForm.tag.trim() && editForm.tag.trim() !== target.tag_name) body.tag_name = editForm.tag.trim();
  if (editForm.version.trim() && editForm.version.trim() !== target.latest_version) {
    body.latest_version = editForm.version.trim();
  }
  if (editForm.sha && editForm.sha.toLowerCase() !== target.raw_sha256.toLowerCase()) {
    body.latest_sha256 = editForm.sha.toLowerCase();
  }
  if (editForm.isDefault !== target.is_default) body.is_default = editForm.isDefault;

  if (Object.keys(body).length === 0) {
    editing.value = null;
    return;
  }
  editError.value = null;
  const ok = await run(() => client.updateChannel(target.guid, body));
  if (ok) editing.value = null;
}

async function removeChannel(channel: AdminChannelView): Promise<void> {
  if (
    await askConfirm(`确认删除通道「${channel.tag_name}」？其版本记录与差分将一并删除。`)
  ) {
    await run(() => client.deleteChannel(channel.guid));
  }
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="font-semibold text-highlighted">发版通道</h2>
          <p class="text-sm text-muted">本应用的通道、版本记录与差分</p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            icon="i-lucide-refresh-cw"
            label="刷新"
            :loading="channelsLoading"
            @click="reloadChannels"
          />
          <UButton size="sm" icon="i-lucide-plus" label="新建通道" @click="creating = true" />
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

    <UTable
      v-model:expanded="expanded"
      :data="channels ?? []"
      :columns="columns"
      :loading="channelsLoading"
      :expanded-options="{ getRowCanExpand: () => true }"
    >
      <template #tag_name-cell="{ row }">
        <span class="font-medium">{{ row.original.tag_name }}</span>
      </template>

      <template #latest_version-cell="{ row }">
        <span class="font-mono text-xs">{{ row.original.latest_version }}</span>
      </template>

      <template #raw_sha256-cell="{ row }">
        <HashChip :hash="row.original.raw_sha256" />
      </template>

      <template #raw_size-cell="{ row }">
        <span class="tabular-nums">{{ formatBytes(row.original.raw_size) }}</span>
      </template>

      <template #is_default-cell="{ row }">
        <UBadge
          v-if="row.original.is_default"
          color="primary"
          variant="subtle"
          size="sm"
          label="默认"
        />
        <span v-else class="text-dimmed">—</span>
      </template>

      <template #actions-cell="{ row }">
        <div class="flex items-center justify-end gap-1">
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            :label="isExpanded(row.index) ? '收起' : '版本 / 差分'"
            @click="toggleExpand(row.index)"
          />
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
            @click="removeChannel(row.original)"
          />
        </div>
      </template>

      <template #expanded="{ row }">
        <ChannelDetail :guid="row.original.guid" />
      </template>

      <template #empty>
        <UEmpty
          icon="i-lucide-git-branch"
          title="暂无发版通道"
          description="请先上传资源，再创建通道"
        />
      </template>
    </UTable>

    <UModal v-model:open="creating" title="新建通道" :ui="{ footer: 'justify-end' }">
      <template #body>
        <div class="grid gap-4 md:grid-cols-2">
          <UFormField label="tag 名" required>
            <UInput v-model="createForm.tag_name" class="w-full font-mono" placeholder="stable" />
          </UFormField>
          <UFormField label="最新版本" required>
            <UInput v-model="createForm.latest_version" class="w-full font-mono" placeholder="1.1.0" />
          </UFormField>
        </div>
        <UFormField label="完整包资源（必须先上传）" required class="mt-4">
          <ResourcePicker v-model="createForm.latest_sha256" />
        </UFormField>
        <UCheckbox
          v-model="createForm.is_default"
          class="mt-4"
          label="设为默认通道（同应用其他默认将自动取消）"
        />
        <p v-if="createError" class="mt-2 text-xs text-error">{{ createError }}</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="creating = false" />
        <UButton label="创建" @click="submitCreate" />
      </template>
    </UModal>

    <UModal
      v-model:open="editOpen"
      :title="editing ? `编辑通道：${editing.tag_name}` : '编辑通道'"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <div class="grid gap-4 md:grid-cols-2">
          <UFormField label="tag 名" required>
            <UInput v-model="editForm.tag" class="w-full font-mono" />
          </UFormField>
          <UFormField label="最新版本" required>
            <UInput v-model="editForm.version" class="w-full font-mono" />
          </UFormField>
        </div>
        <UFormField label="完整包资源" class="mt-4">
          <ResourcePicker v-model="editForm.sha" />
        </UFormField>
        <UCheckbox
          v-model="editForm.isDefault"
          class="mt-4"
          label="设为默认（同应用其他通道将自动取消）"
        />
        <p v-if="editError" class="mt-2 text-xs text-error">{{ editError }}</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="editing = null" />
        <UButton label="保存" @click="submitEdit" />
      </template>
    </UModal>
  </UCard>
</template>
