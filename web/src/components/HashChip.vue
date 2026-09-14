<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue';
import { shortHash } from '../lib/format';

const props = withDefaults(defineProps<{ hash: string; head?: number }>(), { head: 12 });

const copied = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

async function copy(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.hash);
    copied.value = true;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      copied.value = false;
    }, 1200);
  } catch {
    // 剪贴板不可用（非安全上下文等）时静默失败
  }
}

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
});
</script>

<template>
  <UTooltip :text="copied ? '已复制' : hash" :delay-duration="200">
    <UButton
      type="button"
      size="xs"
      variant="soft"
      :color="copied ? 'success' : 'neutral'"
      class="font-mono"
      :label="copied ? '已复制' : shortHash(hash, head)"
      @click="copy"
    />
  </UTooltip>
</template>
