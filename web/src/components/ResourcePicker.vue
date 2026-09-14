<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { ResourceView } from '../api/types';
import { useAsync } from '../composables/useAsync';
import { useSession } from '../composables/useSession';
import { formatBytes } from '../lib/format';
import HashChip from './HashChip.vue';

/**
 * 通用资源选择：搜索全局资源库（sha256 前缀过滤）选中即回填。
 * 替代任何手动输入 sha256 的表单场景（通道完整包 / 差分 / 关联等）。
 */
const props = withDefaults(
  defineProps<{
    /** 当前选中的 sha256，null 未选 */
    modelValue: string | null;
    disabled?: boolean;
    /** 允许清除当前选择 */
    allowClear?: boolean;
  }>(),
  { disabled: false, allowClear: false },
);

const emit = defineEmits<{ 'update:modelValue': [value: string | null] }>();

const { client } = useSession();
const open = ref(false);
const query = ref('');
const picked = ref('');

const {
  data: resources,
  error: resourcesError,
  loading: resourcesLoading,
} = useAsync<ResourceView[]>(
  () => (open.value ? client.listResources() : Promise.resolve([])),
  () => [open.value],
);

watch(open, (value) => {
  if (value) {
    query.value = '';
    picked.value = '';
  }
});

const candidates = computed(() =>
  (resources.value ?? []).filter(
    (resource) => query.value.trim() === '' || resource.sha256.startsWith(query.value.trim().toLowerCase()),
  ),
);

function pick(sha256: string): void {
  emit('update:modelValue', sha256);
  open.value = false;
}
</script>

<template>
  <div class="flex items-center gap-2">
    <div class="flex min-w-0 grow items-center gap-2">
      <HashChip v-if="props.modelValue" :hash="props.modelValue" :head="16" />
      <span v-else class="text-sm text-muted">未选择资源</span>
      <UButton
        v-if="props.modelValue && props.allowClear && !props.disabled"
        size="xs"
        variant="ghost"
        color="error"
        label="清除"
        @click="emit('update:modelValue', null)"
      />
    </div>

    <UButton
      size="sm"
      color="neutral"
      variant="outline"
      label="选择资源"
      :disabled="props.disabled"
      @click="open = true"
    />

    <UModal v-model:open="open" title="选择资源" :ui="{ footer: 'justify-end' }">
      <template #body>
        <UFormField label="搜索资源（sha256 前缀，不输入显示全部）">
          <UInput
            v-model="query"
            class="w-full font-mono"
            placeholder="4b540700c4f1…"
            autofocus
            @update:model-value="picked = ''"
          />
        </UFormField>

        <div v-if="resourcesLoading" class="flex justify-center py-10">
          <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-dimmed" />
        </div>
        <UAlert v-else-if="resourcesError" color="error" variant="subtle" :description="resourcesError" />
        <UEmpty
          v-else-if="(resources ?? []).length === 0"
          icon="i-lucide-package-open"
          title="暂无资源"
          description="请先在「资源」页上传"
        />
        <div v-else class="mt-3 max-h-64 space-y-1 overflow-y-auto rounded-md border border-default p-2">
          <p v-if="candidates.length === 0" class="px-2 py-3 text-center text-sm text-muted">
            未找到匹配的资源
          </p>
          <div
            v-for="resource in candidates"
            :key="resource.sha256"
            role="radio"
            tabindex="0"
            :aria-checked="picked === resource.sha256"
            class="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 transition-colors"
            :class="picked === resource.sha256 ? 'bg-elevated' : 'hover:bg-elevated/50'"
            @click="picked = resource.sha256"
            @keydown.enter="picked = resource.sha256"
          >
            <UIcon
              :name="picked === resource.sha256 ? 'i-lucide-circle-dot' : 'i-lucide-circle'"
              class="size-4 shrink-0 text-dimmed"
            />
            <span @click.stop>
              <HashChip :hash="resource.sha256" :head="10" />
            </span>
            <span class="ms-auto text-xs tabular-nums text-muted">
              {{ resource.name ?? '—' }} · {{ formatBytes(resource.size) }} · {{ resource.pools.join('/') }}
            </span>
          </div>
        </div>
      </template>

      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="open = false" />
        <UButton label="确定" :disabled="picked === ''" @click="pick(picked)" />
      </template>
    </UModal>
  </div>
</template>
