<script setup lang="ts">
import { settleConfirm, useConfirmState } from '../composables/useConfirm';

const state = useConfirmState();
</script>

<template>
  <UModal
    :open="state !== null"
    title="确认操作"
    :close="false"
    :ui="{ footer: 'justify-end' }"
    @update:open="(value: boolean) => !value && settleConfirm(false)"
  >
    <template #body>
      <p class="text-sm">{{ state?.message }}</p>
    </template>
    <template #footer>
      <UButton color="neutral" variant="outline" label="取消" @click="settleConfirm(false)" />
      <UButton color="error" label="确认" @click="settleConfirm(true)" />
    </template>
  </UModal>
</template>
