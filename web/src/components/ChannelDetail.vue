<script setup lang="ts">
import { computed, ref } from 'vue';
import type { AdminDiffView, ReleaseView, UpdateView } from '../api/types';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';
import { formatBytes, formatTime, isSha256 } from '../lib/format';
import HashChip from './HashChip.vue';

/** 展开块：版本记录 + 差分 + 客户端视图预览 */
const props = defineProps<{ guid: string }>();

const { client } = useSession();

const {
  data: releases,
  error: releasesError,
  loading: releasesLoading,
  reload: reloadReleases,
} = useAsync<ReleaseView[]>(() => client.listReleases(props.guid));

const {
  data: diffs,
  error: diffsError,
  loading: diffsLoading,
  reload: reloadDiffs,
} = useAsync<AdminDiffView[]>(() => client.listDiffs(props.guid));

const error = ref<string | null>(null);

const version = ref('');
const releaseSha = ref('');

const baseSha = ref('');
const patchSha = ref('');
const algo = ref('');

const previewOpen = ref(false);

const {
  data: update,
  error: updateError,
  loading: updateLoading,
} = useAsync<UpdateView | null>(
  () => (previewOpen.value ? client.getUpdate(props.guid) : Promise.resolve(null)),
  () => [previewOpen.value],
);

async function run(action: () => Promise<unknown>): Promise<boolean> {
  error.value = null;
  try {
    await action();
    reloadReleases();
    reloadDiffs();
    return true;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

async function addRelease(): Promise<void> {
  if (!version.value.trim() || !isSha256(releaseSha.value)) {
    error.value = '版本必填，sha256 需为 64 位十六进制';
    return;
  }
  const ok = await run(() =>
    client.createRelease(props.guid, { version: version.value.trim(), sha256: releaseSha.value }),
  );
  if (ok) {
    version.value = '';
    releaseSha.value = '';
  }
}

async function removeRelease(release: ReleaseView): Promise<void> {
  if (await askConfirm(`确认删除版本记录 ${release.version}？`)) {
    await run(() => client.deleteRelease(props.guid, release.version));
  }
}

async function addDiff(): Promise<void> {
  if (!isSha256(baseSha.value) || !isSha256(patchSha.value)) {
    error.value = 'base / patch sha256 需为 64 位十六进制';
    return;
  }
  if (baseSha.value === patchSha.value) {
    error.value = 'base 与 patch 不能相同';
    return;
  }
  const ok = await run(() =>
    client.createDiff(props.guid, {
      base_sha256: baseSha.value,
      patch_sha256: patchSha.value,
      algo: algo.value || null,
    }),
  );
  if (ok) {
    baseSha.value = '';
    patchSha.value = '';
    algo.value = '';
  }
}

async function removeDiff(diff: AdminDiffView): Promise<void> {
  if (await askConfirm('确认删除这条差分？')) {
    await run(() => client.deleteDiff(props.guid, diff.base_sha256));
  }
}

const preview = computed(() => update.value ?? null);
</script>

<template>
  <div class="grid gap-6 py-2 whitespace-normal sm:grid-cols-2">
    <div class="min-w-0">
      <div class="mb-2 flex items-baseline justify-between">
        <span class="text-xs tracking-widest text-dimmed uppercase">版本记录 · Releases</span>
        <UButton
          size="xs"
          variant="link"
          label="客户端视图预览"
          @click="previewOpen = true"
        />
      </div>

      <div class="space-y-2">
        <div v-if="releasesLoading" class="flex justify-center py-6">
          <UIcon name="i-lucide-loader-circle" class="size-5 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="releasesError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="releasesError"
        />
        <p v-else-if="(releases ?? []).length === 0" class="text-sm text-muted">暂无版本记录</p>
        <template v-else>
          <div
            v-for="release in releases ?? []"
            :key="release.version"
            class="flex items-center justify-between gap-2 rounded-md bg-elevated px-2 py-1.5"
          >
          <div class="flex min-w-0 items-center gap-2">
            <span class="font-mono text-sm font-semibold">{{ release.version }}</span>
            <HashChip :hash="release.sha256" :head="8" />
          </div>
          <div class="flex items-center gap-2">
            <span class="text-xs text-dimmed">{{ formatTime(release.created_at) }}</span>
            <UButton
              size="xs"
              variant="ghost"
              color="error"
              label="删除"
              @click="removeRelease(release)"
            />
            </div>
          </div>
        </template>
      </div>

      <div class="mt-3 grid grid-cols-[8rem_1fr] gap-2">
        <UInput v-model="version" size="sm" class="font-mono" placeholder="* 版本号（如 1.0.0）" />
        <UInput
          v-model="releaseSha"
          size="sm"
          class="font-mono"
          placeholder="* sha256（需为已上传资源）"
          @keydown.enter="addRelease"
        />
      </div>
      <UButton class="mt-2" size="sm" color="neutral" variant="outline" label="添加版本" @click="addRelease" />
    </div>

    <div class="min-w-0">
      <span class="mb-2 block text-xs tracking-widest text-dimmed uppercase">差分 · Diffs</span>

      <div class="space-y-2">
        <div v-if="diffsLoading" class="flex justify-center py-6">
          <UIcon name="i-lucide-loader-circle" class="size-5 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="diffsError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="diffsError"
        />
        <p v-else-if="(diffs ?? []).length === 0" class="text-sm text-muted">
          暂无差分，客户端将直接下载完整包
        </p>
        <template v-else>
          <div
            v-for="diff in diffs ?? []"
            :key="diff.base_sha256"
            class="flex items-center gap-2 rounded-md bg-elevated px-2 py-1.5"
          >
          <HashChip :hash="diff.base_sha256" :head="8" />
          <UIcon name="i-lucide-move-right" class="size-4 text-primary" />
          <HashChip :hash="diff.patch_sha256" :head="8" />
          <span class="ms-auto flex items-center gap-2 text-xs text-muted">
            <span>{{ diff.algo ?? '—' }}</span>
            <span class="tabular-nums">{{ formatBytes(diff.size) }}</span>
            <UButton
              size="xs"
              variant="ghost"
              color="error"
              label="删除"
              @click="removeDiff(diff)"
            />
            </span>
          </div>
        </template>
      </div>

      <div class="mt-3 grid grid-cols-2 gap-2">
        <UInput v-model="baseSha" size="sm" class="font-mono" placeholder="* base sha256" />
        <UInput
          v-model="patchSha"
          size="sm"
          class="font-mono"
          placeholder="* patch sha256"
          @keydown.enter="addDiff"
        />
      </div>
      <div class="mt-2 flex gap-2">
        <UInput
          v-model="algo"
          size="sm"
          class="font-mono"
          placeholder="算法（可选，如 xdelta3）"
        />
        <UButton size="sm" color="neutral" variant="outline" label="添加差分" @click="addDiff" />
      </div>
    </div>

    <UAlert
      v-if="error"
      class="sm:col-span-2"
      color="error"
      variant="subtle"
      icon="i-lucide-triangle-alert"
      :description="error"
    />

    <!-- 客户端视角：同一个通道，普通客户端通过 X-App-Id 拿到的响应 -->
    <UModal v-model:open="previewOpen" title="客户端视图预览">
      <template #body>
        <div v-if="updateLoading" class="flex justify-center py-10">
          <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="updateError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="updateError"
        />
        <div v-else-if="preview" class="space-y-4">
          <div class="flex items-baseline gap-2">
            <span class="text-sm text-muted">{{ preview.tag_name }}</span>
            <span class="font-mono text-lg font-bold">{{ preview.version }}</span>
          </div>
          <div class="flex items-center gap-2 text-sm">
            <span class="text-muted">完整包</span>
            <HashChip :hash="preview.raw_sha256" />
            <span class="tabular-nums text-muted">{{ formatBytes(preview.raw_size) }}</span>
          </div>
          <div>
            <span class="block py-1 text-xs tracking-widest text-dimmed uppercase">
              客户端以本地 sha256 比对并选择差分
            </span>
            <p v-if="preview.diffs.length === 0" class="text-sm text-muted">
              暂无差分，客户端将直接下载完整包
            </p>
            <div v-else class="space-y-1.5">
              <div v-for="diff in preview.diffs" :key="diff.base_sha256" class="flex items-center gap-2">
                <HashChip :hash="diff.base_sha256" :head="8" />
                <UIcon name="i-lucide-move-right" class="size-4 text-primary" />
                <HashChip :hash="diff.patch_sha256" :head="8" />
                <span class="ms-auto text-xs text-muted">{{ diff.algo ?? '—' }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
