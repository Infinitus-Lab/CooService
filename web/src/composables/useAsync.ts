import { ref, watch, type Ref } from 'vue';
import { ApiError } from '../api/client';

/**
 * 触发一次请求，暴露 data / error / loading / reload。
 * deps 传 getter（如 () => [filter.value]），其依赖变化时自动重跑。
 */
export function useAsync<T>(fn: () => Promise<T>, deps: () => unknown[] = () => []) {
  const data: Ref<T | null> = ref<T | null>(null) as Ref<T | null>;
  const error = ref<string | null>(null);
  const loading = ref(false);
  let seq = 0;

  async function run(): Promise<void> {
    const id = ++seq;
    loading.value = true;
    error.value = null;
    try {
      const result = await fn();
      if (id === seq) data.value = result;
    } catch (e) {
      if (id === seq) error.value = e instanceof ApiError ? e.message : String(e);
    } finally {
      if (id === seq) loading.value = false;
    }
  }

  watch(deps, () => void run(), { immediate: true });

  return { data, error, loading, reload: () => void run() };
}
