import { ref } from 'vue';
import { createClient, type ApiClient } from '../api/client';

const KEY_STORAGE = 'coo.admin_key';
const BASE_STORAGE = 'coo.api_base';

const adminKey = ref(sessionStorage.getItem(KEY_STORAGE) ?? '');
const baseApi = ref(sessionStorage.getItem(BASE_STORAGE) ?? '');
/** 全屏门：地址不可达或管理接口 401 时打开 */
const gateOpen = ref(false);
const gateFocus = ref<'base' | 'key'>('base');
/** 每次重新验证配置后自增，用于整体重挂载业务视图 */
const epoch = ref(0);

function onUnauthorized(): void {
  gateFocus.value = 'key';
  gateOpen.value = true;
}

function onUnreachable(): void {
  gateFocus.value = 'base';
  gateOpen.value = true;
}

const client: ApiClient = createClient(
  () => baseApi.value,
  () => adminKey.value,
  onUnauthorized,
  onUnreachable,
);

/** 探活成功才落盘；地址错聚焦地址框，密钥错聚焦密钥框（由 ApiSetupDialog 判定） */
async function verify(base: string, key: string): Promise<void> {
  const probe = createClient(
    () => base,
    () => key,
    () => {},
    () => {},
  );
  await probe.listApps();
  sessionStorage.setItem(BASE_STORAGE, base);
  sessionStorage.setItem(KEY_STORAGE, key);
  baseApi.value = base;
  adminKey.value = key;
  gateOpen.value = false;
  epoch.value += 1;
}

/** 断开只清密钥，地址保留 */
function disconnect(): void {
  sessionStorage.removeItem(KEY_STORAGE);
  adminKey.value = '';
  epoch.value += 1;
}

export function useSession() {
  return { adminKey, baseApi, gateOpen, gateFocus, epoch, client, verify, disconnect };
}
