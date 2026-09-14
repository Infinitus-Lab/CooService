import { shallowRef } from 'vue';

interface ConfirmState {
  message: string;
  resolve: (ok: boolean) => void;
}

const state = shallowRef<ConfirmState | null>(null);

/** 全局统一确认弹窗（替代浏览器原生 confirm），由 App 里的 ConfirmDialog 渲染 */
export function askConfirm(message: string): Promise<boolean> {
  return new Promise<boolean>((resolve) => {
    state.value = { message, resolve };
  });
}

export function settleConfirm(ok: boolean): void {
  const current = state.value;
  state.value = null;
  current?.resolve(ok);
}

export function useConfirmState() {
  return state;
}
