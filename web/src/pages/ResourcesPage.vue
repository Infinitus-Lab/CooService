<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { ResourceDetailView, ResourceView } from '../api/types';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';
import { formatBytes, formatTime } from '../lib/format';
import HashChip from '../components/HashChip.vue';

/** 资源库：内容寻址、永远先落本地；进池/同步是资源池管理的事。 */
const { client } = useSession();

const {
  data: resources,
  error: resourcesError,
  loading: resourcesLoading,
  reload: reloadResources,
} = useAsync<ResourceView[]>(() => client.listResources());

const actionError = ref<string | null>(null);
const displayError = computed(() => actionError.value ?? resourcesError.value);

const columns = [
  { id: 'name', header: '显示名' },
  { id: 'sha256', header: '指纹' },
  { id: 'size', header: '大小', meta: { class: { th: 'text-right', td: 'text-right' } } },
  { id: 'created_at', header: '上传于' },
  { id: 'pools', header: '所在池' },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

// ---------- 上传 ----------

const uploadOpen = ref(false);
const uploadFile = ref<File | null>(null);
const uploadName = ref('');
const uploading = ref(false);
const uploadError = ref<string | null>(null);

watch(uploadOpen, (open) => {
  if (open) {
    uploadFile.value = null;
    uploadName.value = '';
    uploadError.value = null;
  }
});

async function startUpload(): Promise<void> {
  const file = uploadFile.value;
  if (!file || uploading.value) return;
  uploading.value = true;
  uploadError.value = null;
  try {
    const result = await client.uploadResource(file, uploadName.value.trim() || undefined);
    uploadOpen.value = false;
    reloadResources();
    detailSha.value = result.sha256;
  } catch (e) {
    uploadError.value = e instanceof Error ? e.message : String(e);
  } finally {
    uploading.value = false;
  }
}

// ---------- 详情 ----------

const detailSha = ref<string | null>(null);
const renaming = ref(false);
const renameDraft = ref('');
const renameError = ref<string | null>(null);

const detailOpen = computed({
  get: () => detailSha.value !== null,
  set: (value) => {
    if (!value) {
      detailSha.value = null;
      renaming.value = false;
      renameError.value = null;
    }
  },
});

const {
  data: detail,
  error: detailError,
  loading: detailLoading,
  reload: reloadDetail,
} = useAsync<ResourceDetailView | null>(
  () => (detailSha.value === null ? Promise.resolve(null) : client.getResourceDetail(detailSha.value)),
  () => [detailSha.value],
);

function beginRename(): void {
  renameDraft.value = detail.value?.name ?? '';
  renaming.value = true;
  renameError.value = null;
}

async function saveRename(): Promise<void> {
  const sha = detailSha.value;
  if (!sha) return;
  renameError.value = null;
  try {
    await client.renameResource(sha, renameDraft.value.trim() || undefined);
    renaming.value = false;
    reloadDetail();
    reloadResources();
  } catch (e) {
    renameError.value = e instanceof Error ? e.message : String(e);
  }
}

async function removeResource(resource: ResourceView): Promise<void> {
  if (!(await askConfirm('确认删除该资源？本地副本与资源池副本将一并删除。'))) return;
  actionError.value = null;
  try {
    await client.deleteResource(resource.sha256);
    reloadResources();
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-4">
        <div>
          <h2 class="font-semibold text-highlighted">资源</h2>
          <p class="text-sm text-muted">本地上传（内容寻址）；进入资源池与同步由「资源池」页负责</p>
        </div>
        <div class="flex shrink-0 gap-2">
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            icon="i-lucide-refresh-cw"
            label="刷新"
            :loading="resourcesLoading"
            @click="reloadResources"
          />
          <UButton size="sm" icon="i-lucide-upload" label="上传资源" @click="uploadOpen = true" />
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

    <UTable :data="resources ?? []" :columns="columns" :loading="resourcesLoading">
      <template #name-cell="{ row }">
        <span class="font-medium">{{ row.original.name ?? '—' }}</span>
      </template>

      <template #sha256-cell="{ row }">
        <HashChip :hash="row.original.sha256" />
      </template>

      <template #size-cell="{ row }">
        <span class="tabular-nums">{{ formatBytes(row.original.size) }}</span>
      </template>

      <template #created_at-cell="{ row }">
        <span class="text-xs text-muted">{{ formatTime(row.original.created_at) }}</span>
      </template>

      <template #pools-cell="{ row }">
        <UBadge
          v-if="row.original.pools.length === 0"
          color="warning"
          variant="subtle"
          size="sm"
          label="仅本地"
        />
        <div v-else class="flex flex-wrap gap-1">
          <UBadge
            v-for="pool in row.original.pools"
            :key="pool"
            color="neutral"
            variant="subtle"
            size="sm"
            class="font-mono"
            :label="pool"
          />
        </div>
      </template>

      <template #actions-cell="{ row }">
        <div class="flex items-center justify-end gap-1">
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            label="详情"
            @click="detailSha = row.original.sha256"
          />
          <UButton
            size="xs"
            color="error"
            variant="ghost"
            label="删除"
            @click="removeResource(row.original)"
          />
        </div>
      </template>

      <template #empty>
        <UEmpty
          icon="i-lucide-package-open"
          title="暂无资源"
          description="点击右上角「上传资源」上传第一个资源"
        />
      </template>
    </UTable>

    <!-- 上传：选择文件 → 设定资源名 → 开始 -->
    <UModal
      v-model:open="uploadOpen"
      title="上传资源"
      :dismissible="!uploading"
      :close="!uploading"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <UFormField label="资源名" hint="可选，可后续修改">
          <UInput
            v-model="uploadName"
            class="w-full font-mono"
            placeholder="仅管理端识别用"
            :disabled="uploading"
          />
        </UFormField>

        <UFileUpload
          v-model="uploadFile"
          class="mt-4"
          variant="area"
          icon="i-lucide-upload"
          label="拖拽文件到此处，或点击选择"
          description="单文件上传；再次选择即替换"
          :disabled="uploading"
        />

        <p v-if="uploadError" class="mt-2 text-xs text-error">{{ uploadError }}</p>
      </template>
      <template #footer>
        <UButton
          color="neutral"
          variant="outline"
          label="取消"
          :disabled="uploading"
          @click="uploadOpen = false"
        />
        <UButton
          :loading="uploading"
          :disabled="!uploadFile"
          :label="uploading ? '上传中…' : '开始上传'"
          @click="startUpload"
        />
      </template>
    </UModal>

    <!-- 详情 -->
    <UModal v-model:open="detailOpen" title="资源详情">
      <template #body>
        <div v-if="detailLoading" class="flex justify-center py-10">
          <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="detailError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="detailError"
        />
        <div v-else-if="detail" class="space-y-4">
          <div class="flex items-baseline gap-2">
            <UInput
              v-if="renaming"
              v-model="renameDraft"
              size="sm"
              class="w-64 font-mono"
              autofocus
              @keydown.enter="saveRename"
            />
            <span v-else class="text-lg font-bold text-highlighted">
              {{ detail.name ?? '（未命名）' }}
            </span>
            <UButton
              v-if="renaming"
              size="xs"
              label="保存"
              @click="saveRename"
            />
            <UButton
              v-else
              size="xs"
              variant="ghost"
              color="neutral"
              icon="i-lucide-pencil"
              label="重命名"
              @click="beginRename"
            />
            <span class="ms-auto text-sm tabular-nums text-muted">
              {{ formatBytes(detail.size) }}
            </span>
          </div>
          <p v-if="renaming && renameError" class="text-xs text-error">{{ renameError }}</p>

          <div>
            <span class="block py-1 text-xs tracking-widest text-dimmed uppercase">
              sha256 · 完整指纹可复制
            </span>
            <p class="font-mono text-xs break-all text-muted">{{ detail.sha256 }}</p>
          </div>

          <div class="grid grid-cols-2 gap-3 text-sm">
            <div>
              <span class="block text-xs text-muted">上传时间</span>
              <span class="font-mono">{{ formatTime(detail.created_at) }}</span>
            </div>
            <div>
              <span class="block text-xs text-muted">所在资源池</span>
              <UBadge v-if="detail.pools.length === 0" color="warning" variant="subtle" size="sm" label="仅本地" />
              <span v-else class="font-mono text-xs">{{ detail.pools.join(' / ') }}</span>
            </div>
          </div>

          <div>
            <span class="block text-xs text-muted">引用应用</span>
            <span v-if="detail.ref_apps.length === 0" class="text-sm text-muted">无</span>
            <span v-else class="text-sm">{{ detail.ref_apps.join('、') }}</span>
          </div>
        </div>
      </template>
    </UModal>
  </UCard>
</template>
