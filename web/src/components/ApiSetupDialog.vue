<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue';
import { ApiError } from '../api/client';
import { useSession } from '../composables/useSession';

/**
 * API 设置门：同屏输入 API 地址与管理密钥，探活成功才放行。
 * 强制 Dialog——透明遮罩，无关闭按钮/无 Esc，地址不可达或密钥 401 时业务页不可用。
 */
const props = defineProps<{
  /** 打开时聚焦的字段：地址不可达 → 'base'，密钥 401 → 'key'，手动打开 → 'base' */
  focus: 'base' | 'key';
}>();

const session = useSession();
const base = ref(session.baseApi.value);
const key = ref(session.adminKey.value);
const checking = ref(false);
const error = ref<string | null>(null);

const baseValid = computed(() => base.value.trim() === '' || /^https?:\/\//i.test(base.value.trim()));
const canSubmit = computed(() => baseValid.value && key.value.trim() !== '' && !checking.value);

async function submit(): Promise<void> {
  if (!canSubmit.value) return;
  checking.value = true;
  error.value = null;
  try {
    await session.verify(base.value.trim(), key.value.trim());
  } catch (e) {
    if (e instanceof ApiError && e.code === 401) {
      error.value = '密钥无效，请确认与部署方提供的管理密钥一致';
      document.getElementById('api-setup-key')?.focus();
    } else if (e instanceof ApiError && e.code === 0) {
      error.value = e.message;
      document.getElementById('api-setup-base')?.focus();
    } else {
      error.value = e instanceof Error ? e.message : String(e);
    }
  } finally {
    checking.value = false;
  }
}

const inputClass = 'w-full font-mono';

// Reka 的 Dialog 默认聚焦第一个可聚焦元素，这里按 focus 明确指定要聚焦的输入框
onMounted(() => {
  void nextTick(() => {
    const id = props.focus === 'key' ? 'api-setup-key' : 'api-setup-base';
    document.getElementById(id)?.focus();
  });
});
</script>

<template>
  <UModal :open="true" :dismissible="false" :close="false">
    <template #body>
      <div class="mb-1 flex items-baseline justify-between">
        <span class="font-mono text-sm font-bold tracking-[0.22em]">COOSERVICE · API 设置</span>
        <span class="text-xs text-muted">X-Admin-Key</span>
      </div>
      <p class="mb-4 text-sm text-muted">
        配置 API 地址与管理密钥；地址留空表示同源。仅存于当前标签页（sessionStorage）
      </p>

      <div class="space-y-4">
        <UFormField
          label="API 地址"
          :error="base.trim() !== '' && !baseValid ? '地址需以 http:// 或 https:// 开头' : undefined"
        >
          <UInput
            id="api-setup-base"
            v-model="base"
            :class="inputClass"
            placeholder="http://127.0.0.1:8081（留空 = 同源）"
            @keydown.enter="submit"
          />
        </UFormField>

        <UFormField label="管理密钥">
          <UInput
            id="api-setup-key"
            v-model="key"
            type="password"
            :class="inputClass"
            placeholder="粘贴管理密钥"
            @keydown.enter="submit"
          />
        </UFormField>
      </div>

      <p v-if="error" class="mt-2 text-xs text-error">{{ error }}</p>

      <UButton
        class="mt-4 w-full justify-center"
        :loading="checking"
        :disabled="!canSubmit"
        :label="checking ? '验证中…' : '保存并验证'"
        @click="submit"
      />
    </template>
  </UModal>
</template>
