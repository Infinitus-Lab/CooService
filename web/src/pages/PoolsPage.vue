<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type { PoolResourceView, PoolView, ResourceView } from '../api/types';
import HashChip from '../components/HashChip.vue';
import { useAsync } from '../composables/useAsync';
import { askConfirm } from '../composables/useConfirm';
import { useSession } from '../composables/useSession';
import { formatBytes, formatTime } from '../lib/format';

interface S3Form {
  bucket: string;
  region: string;
  access_key: string;
  secret: string;
  path_style: boolean;
}

interface FtpForm {
  user: string;
  password: string;
  root: string;
  port: string;
}

const { client } = useSession();

const {
  data: pools,
  error: poolsError,
  loading: poolsLoading,
  reload: reloadPools,
} = useAsync<PoolView[]>(() => client.listPools());

const actionError = ref<string | null>(null);
const displayError = computed(() => actionError.value ?? poolsError.value);

const columns = [
  { id: 'id', header: 'Id' },
  { id: 'kind', header: '类型' },
  { id: 'endpoint', header: '终结点' },
  { id: 'summary', header: '参数' },
  { id: 'health', header: '健康' },
  { id: 'sync', header: '同步' },
  { id: 'is_public', header: '公开下载' },
  { id: 'actions', header: '操作', meta: { class: { th: 'text-right', td: 'text-right' } } },
];

/** 池列表里的参数摘要（s3 → bucket @ region，ftp → user@host:port） */
function poolSummary(pool: PoolView): string {
  if (pool.kind === 's3') {
    const config = pool.config as Partial<S3Form>;
    return [config.bucket, config.region].filter(Boolean).join(' @ ') || '—';
  }
  const config = pool.config as Partial<FtpForm>;
  const host = pool.endpoint.replace(/^https?:\/\//, '');
  return `${config.user || '?'}@${host}${config.port ? `:${config.port}` : ''}`;
}

function healthBadge(pool: PoolView): { color: 'success' | 'error' | 'neutral'; label: string } {
  if (!pool.connected) return { color: 'neutral', label: '未连接' };
  return pool.up ? { color: 'success', label: '健康' } : { color: 'error', label: '异常' };
}

function healthTitle(pool: PoolView): string {
  if (!pool.connected) return '运行时注册表里没有连接';
  return pool.up
    ? `${pool.health_checked_at ?? '—'} · 探测通过`
    : (pool.health_error ?? '探测失败');
}

function syncBadge(pool: PoolView): { color: 'success' | 'warning' | 'error' | 'neutral'; label: string } {
  switch (pool.sync_status) {
    case 'synced':
      return { color: 'success', label: '已同步' };
    case 'syncing':
      return { color: 'warning', label: `同步中（${pool.sync_pending}）` };
    case 'error':
      return { color: 'error', label: '同步出错' };
    default:
      return { color: 'neutral', label: '未同步' };
  }
}

async function run(action: () => Promise<unknown>): Promise<boolean> {
  actionError.value = null;
  try {
    await action();
    reloadPools();
    return true;
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

// ---------- 强制全扫描 ----------

const scanning = ref(false);

async function scan(): Promise<void> {
  scanning.value = true;
  actionError.value = null;
  try {
    await client.scanPools();
    reloadPools();
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : String(e);
  } finally {
    scanning.value = false;
  }
}

async function togglePublic(pool: PoolView): Promise<void> {
  await run(() => client.updatePool(pool.id, { is_public: !pool.is_public }));
}

async function removePool(pool: PoolView): Promise<void> {
  if (await askConfirm(`确认删除资源池「${pool.id}」？`)) {
    await run(() => client.deletePool(pool.id));
  }
}

// ---------- 新建 ----------

const creating = ref(false);
const createError = ref<string | null>(null);
const kind = ref<'s3' | 'ftp'>('s3');

function emptyCreateForm() {
  return {
    id: '',
    endpoint: '',
    public_endpoint: '',
    is_public: false,
    s3: { bucket: '', region: '', access_key: '', secret: '', path_style: true } as S3Form,
    ftp: { user: '', password: '', root: '', port: '21' } as FtpForm,
  };
}

const createForm = reactive(emptyCreateForm());

watch(creating, (open) => {
  if (open) {
    Object.assign(createForm, emptyCreateForm());
    kind.value = 's3';
    createError.value = null;
  }
});

function validateCreate(): string | null {
  if (!createForm.id.trim()) return 'Id 必填';
  if (kind.value === 's3') {
    if (
      !createForm.s3.bucket.trim() ||
      !createForm.s3.region.trim() ||
      !createForm.s3.access_key.trim() ||
      !createForm.s3.secret
    ) {
      return 'S3 需要 bucket、region、access key 和 secret key';
    }
  } else if (!createForm.ftp.user.trim() || !createForm.ftp.password) {
    return 'FTP 需要用户名和密码';
  }
  if (createForm.is_public && !createForm.public_endpoint.trim()) {
    return '公开下载需要填写公开终结点';
  }
  if (createForm.ftp.port.trim() && Number.isNaN(Number(createForm.ftp.port))) {
    return 'FTP 端口必须是数字';
  }
  return null;
}

async function submitCreate(): Promise<void> {
  const message = validateCreate();
  if (message) {
    createError.value = message;
    return;
  }
  createError.value = null;
  // 按类型组装提交用的 config / secret
  const body = {
    id: createForm.id.trim(),
    kind: kind.value,
    endpoint: createForm.endpoint.trim(),
    public_endpoint: createForm.public_endpoint.trim(),
    is_public: createForm.is_public,
    secret: kind.value === 's3' ? createForm.s3.secret : createForm.ftp.password,
    config:
      kind.value === 's3'
        ? {
            bucket: createForm.s3.bucket.trim(),
            region: createForm.s3.region.trim(),
            access_key: createForm.s3.access_key.trim(),
            path_style: createForm.s3.path_style,
          }
        : {
            user: createForm.ftp.user.trim(),
            root: createForm.ftp.root.trim() || undefined,
            port: createForm.ftp.port.trim() ? Number(createForm.ftp.port) : undefined,
          },
  };
  if (await run(() => client.createPool(body))) creating.value = false;
}

// ---------- 编辑 ----------

const editing = ref<PoolView | null>(null);
const editError = ref<string | null>(null);
const editForm = reactive({
  endpoint: '',
  publicEndpoint: '',
  isPublic: false,
  s3: null as S3Form | null,
  ftp: null as FtpForm | null,
});

watch(editing, (pool) => {
  if (!pool) return;
  editForm.endpoint = pool.endpoint;
  editForm.publicEndpoint = pool.public_endpoint;
  editForm.isPublic = pool.is_public;
  editForm.s3 =
    pool.kind === 's3'
      ? {
          bucket: (pool.config.bucket as string) ?? '',
          region: (pool.config.region as string) ?? '',
          access_key: (pool.config.access_key as string) ?? '',
          secret: '',
          path_style: (pool.config.path_style as boolean) ?? true,
        }
      : null;
  editForm.ftp =
    pool.kind === 'ftp'
      ? {
          user: (pool.config.user as string) ?? '',
          password: '',
          root: (pool.config.root as string) ?? '',
          port: String((pool.config.port as number) ?? 21),
        }
      : null;
  editError.value = null;
});

const editOpen = computed({
  get: () => editing.value !== null,
  set: (value) => {
    if (!value) editing.value = null;
  },
});

function currentConfig(): Record<string, unknown> {
  const target = editing.value;
  if (!target) return {};
  if (target.kind === 's3' && editForm.s3) {
    return {
      bucket: editForm.s3.bucket.trim(),
      region: editForm.s3.region.trim(),
      access_key: editForm.s3.access_key.trim(),
      path_style: editForm.s3.path_style,
    };
  }
  if (editForm.ftp) {
    return {
      user: editForm.ftp.user.trim(),
      root: editForm.ftp.root.trim() || undefined,
      port: editForm.ftp.port.trim() ? Number(editForm.ftp.port) : undefined,
    };
  }
  return {};
}

async function submitEdit(): Promise<void> {
  const target = editing.value;
  if (!target) return;
  const isS3 = target.kind === 's3';
  if (isS3 && (!editForm.s3?.bucket.trim() || !editForm.s3.region.trim() || !editForm.s3.access_key.trim())) {
    editError.value = 'S3 需要 bucket、region、access key';
    return;
  }
  if (!isS3 && !editForm.ftp?.user.trim()) {
    editError.value = 'FTP 需要用户名';
    return;
  }
  if (editForm.isPublic && !editForm.publicEndpoint.trim()) {
    editError.value = '公开下载需要填写公开终结点';
    return;
  }

  const config = currentConfig();
  const configChanged =
    JSON.stringify(config, Object.keys(config).sort()) !==
    JSON.stringify(target.config, Object.keys(target.config).sort());

  const body: Record<string, unknown> = {};
  if (editForm.endpoint !== target.endpoint) body.endpoint = editForm.endpoint.trim();
  if (editForm.publicEndpoint !== target.public_endpoint) {
    body.public_endpoint = editForm.publicEndpoint.trim();
  }
  if (editForm.isPublic !== target.is_public) body.is_public = editForm.isPublic;
  if (configChanged) body.config = config;

  if (Object.keys(body).length === 0) {
    editing.value = null;
    return;
  }
  editError.value = null;
  if (await run(() => client.updatePool(target.id, body))) editing.value = null;
}

// ---------- 改密钥 ----------

const rotating = ref<PoolView | null>(null);
const rotatedSecret = ref('');

watch(rotating, (pool) => {
  if (pool) rotatedSecret.value = '';
});

const rotateOpen = computed({
  get: () => rotating.value !== null,
  set: (value) => {
    if (!value) rotating.value = null;
  },
});

async function submitRotate(): Promise<void> {
  const target = rotating.value;
  if (!target || rotatedSecret.value === '') return;
  if (await run(() => client.updatePool(target.id, { secret: rotatedSecret.value }))) {
    rotating.value = null;
  }
}

// ---------- 池内资源 ----------

const poolDetail = ref<PoolView | null>(null);
const memberError = ref<string | null>(null);
const selectedMember = ref('');

const {
  data: members,
  error: membersError,
  loading: membersLoading,
  reload: reloadMembers,
} = useAsync<PoolResourceView[]>(
  () => (poolDetail.value ? client.poolResources(poolDetail.value.id) : Promise.resolve([])),
  () => [poolDetail.value],
);

const {
  data: allResources,
  loading: allResourcesLoading,
} = useAsync<ResourceView[]>(
  () => (poolDetail.value ? client.listResources() : Promise.resolve([])),
  () => [poolDetail.value],
);

watch(poolDetail, (pool) => {
  if (pool) {
    selectedMember.value = '';
    memberError.value = null;
  }
});

const detailOpen = computed({
  get: () => poolDetail.value !== null,
  set: (value) => {
    if (!value) poolDetail.value = null;
  },
});

const memberCandidates = computed(() =>
  (allResources.value ?? []).filter(
    (resource) => !(members.value ?? []).some((member) => member.sha256 === resource.sha256),
  ),
);

const memberOptions = computed(() =>
  memberCandidates.value.map((resource) => ({
    label: `${resource.name ?? resource.sha256.slice(0, 12)} · ${resource.sha256.slice(0, 12)}`,
    value: resource.sha256,
  })),
);

async function addMember(): Promise<void> {
  const pool = poolDetail.value;
  if (!pool || !selectedMember.value) return;
  memberError.value = null;
  try {
    await client.poolAddResource(pool.id, selectedMember.value);
    selectedMember.value = '';
    reloadMembers();
  } catch (e) {
    memberError.value = e instanceof Error ? e.message : String(e);
  }
}

async function removeMember(member: PoolResourceView): Promise<void> {
  const pool = poolDetail.value;
  if (!pool) return;
  if (!(await askConfirm('确认从该池移除？本地副本与登记不受影响。'))) return;
  memberError.value = null;
  try {
    await client.poolRemoveResource(pool.id, member.sha256);
    reloadMembers();
  } catch (e) {
    memberError.value = e instanceof Error ? e.message : String(e);
  }
}

function poolMenu(pool: PoolView) {
  return [
    [
      { label: '池内资源', icon: 'i-lucide-package', onSelect: () => (poolDetail.value = pool) },
      { label: '编辑', icon: 'i-lucide-pencil', onSelect: () => (editing.value = pool) },
      { label: '改密钥', icon: 'i-lucide-key-round', onSelect: () => (rotating.value = pool) },
    ],
    [
      {
        label: '删除',
        icon: 'i-lucide-trash',
        color: 'error' as const,
        onSelect: () => void removePool(pool),
      },
    ],
  ];
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="font-semibold text-highlighted">资源池</h2>
          <p class="text-sm text-muted">
            仅保存配置；密钥不回显，仅可重设；公开池参与下载重定向
          </p>
        </div>
        <div class="flex gap-2">
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            icon="i-lucide-refresh-cw"
            label="刷新"
            :loading="poolsLoading"
            @click="reloadPools"
          />
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            icon="i-lucide-radar"
            :label="scanning ? '触发中…' : '强制扫描'"
            :disabled="scanning"
            @click="scan"
          />
          <UButton size="sm" icon="i-lucide-plus" label="新建资源池" @click="creating = true" />
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

    <UTable :data="pools ?? []" :columns="columns" :loading="poolsLoading">
      <template #id-cell="{ row }">
        <span class="font-mono text-xs">{{ row.original.id }}</span>
      </template>

      <template #kind-cell="{ row }">
        <UBadge color="neutral" variant="outline" size="sm" :label="row.original.kind" />
      </template>

      <template #endpoint-cell="{ row }">
        <span class="block max-w-56 truncate font-mono text-xs text-muted">
          {{ row.original.endpoint }}
        </span>
      </template>

      <template #summary-cell="{ row }">
        <span class="block max-w-48 truncate font-mono text-xs text-muted">
          {{ poolSummary(row.original) }}
        </span>
      </template>

      <template #health-cell="{ row }">
        <UTooltip :text="healthTitle(row.original)">
          <UBadge
            :color="healthBadge(row.original).color"
            variant="subtle"
            size="sm"
            :label="healthBadge(row.original).label"
          />
        </UTooltip>
      </template>

      <template #sync-cell="{ row }">
        <div>
          <UTooltip :text="row.original.sync_error ?? '无同步错误'">
            <UBadge
              :color="syncBadge(row.original).color"
              variant="subtle"
              size="sm"
              :label="syncBadge(row.original).label"
            />
          </UTooltip>
          <div class="mt-0.5 text-xs text-muted">
            {{ row.original.object_count }} 项 · {{ formatBytes(row.original.total_size) }}
          </div>
        </div>
      </template>

      <template #is_public-cell="{ row }">
        <div class="flex items-center gap-2">
          <USwitch
            :model-value="row.original.is_public"
            size="sm"
            @update:model-value="togglePublic(row.original)"
          />
          <span class="text-xs">{{ row.original.is_public ? '公开' : '内部' }}</span>
        </div>
      </template>

      <template #actions-cell="{ row }">
        <div class="flex justify-end">
          <UDropdownMenu :items="poolMenu(row.original)">
            <UButton size="xs" color="neutral" variant="outline" label="操作" />
          </UDropdownMenu>
        </div>
      </template>

      <template #empty>
        <UEmpty
          icon="i-lucide-database"
          title="暂无资源池"
          description="点击右上角「新建资源池」创建第一个资源池"
        />
      </template>
    </UTable>

    <!-- 新建 -->
    <UModal v-model:open="creating" title="新建资源池" :ui="{ footer: 'justify-end' }">
      <template #body>
        <div class="grid gap-4 md:grid-cols-2">
          <UFormField label="Id" required>
            <UInput v-model="createForm.id" class="w-full font-mono" />
          </UFormField>
          <UFormField label="类型">
            <USelect
              v-model="kind"
              :items="[
                { label: 's3', value: 's3' },
                { label: 'ftp', value: 'ftp' },
              ]"
              class="w-full"
            />
          </UFormField>
        </div>

        <div class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="终结点" required>
            <UInput
              v-model="createForm.endpoint"
              class="w-full font-mono"
              placeholder="s3: http://minio:9000 / ftp: ftp.example.com"
            />
          </UFormField>
          <UFormField label="公开终结点">
            <UInput
              v-model="createForm.public_endpoint"
              class="w-full font-mono"
              placeholder="https://dl.example.com"
            />
          </UFormField>
        </div>

        <UCheckbox
          v-model="createForm.is_public"
          class="mt-4"
          label="公开下载池（被选为 302 重定向目标，需匿名可读）"
        />

        <!-- 字段组随类型切换 -->
        <div v-if="kind === 's3'" class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="Bucket" required>
            <UInput v-model="createForm.s3.bucket" class="w-full font-mono" placeholder="coo" />
          </UFormField>
          <UFormField label="Region" required>
            <UInput v-model="createForm.s3.region" class="w-full font-mono" placeholder="us-east-1" />
          </UFormField>
          <UFormField label="Access Key" required>
            <UInput v-model="createForm.s3.access_key" class="w-full font-mono" placeholder="access id" />
          </UFormField>
          <UFormField label="Secret Key" required>
            <UInput
              v-model="createForm.s3.secret"
              type="password"
              class="w-full font-mono"
              placeholder="access token"
            />
          </UFormField>
          <UCheckbox
            v-model="createForm.s3.path_style"
            class="md:col-span-2"
            label="Path Style（默认开启，适配 MinIO 等自托管对象存储）"
          />
        </div>

        <div v-else class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="用户名" required>
            <UInput v-model="createForm.ftp.user" class="w-full font-mono" placeholder="ftp user" />
          </UFormField>
          <UFormField label="密码" required>
            <UInput v-model="createForm.ftp.password" type="password" class="w-full font-mono" />
          </UFormField>
          <UFormField label="根目录" hint="可选">
            <UInput v-model="createForm.ftp.root" class="w-full font-mono" placeholder="/coo" />
          </UFormField>
          <UFormField label="端口">
            <UInput v-model="createForm.ftp.port" class="w-full font-mono" />
          </UFormField>
        </div>

        <p v-if="createError" class="mt-2 text-xs text-error">{{ createError }}</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="creating = false" />
        <UButton label="创建" @click="submitCreate" />
      </template>
    </UModal>

    <!-- 编辑 -->
    <UModal
      v-model:open="editOpen"
      :title="editing ? `编辑资源池：${editing.id}` : '编辑资源池'"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <div v-if="editing" class="mb-3 flex items-center gap-2">
          <UBadge color="neutral" variant="outline" size="sm" :label="editing.kind" />
          <span class="text-xs text-muted">类型不可变更；密钥修改请使用列表中的「改密钥」</span>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <UFormField label="终结点" required>
            <UInput v-model="editForm.endpoint" class="w-full font-mono" />
          </UFormField>
          <UFormField label="公开终结点">
            <UInput v-model="editForm.publicEndpoint" class="w-full font-mono" />
          </UFormField>
        </div>

        <UCheckbox
          v-model="editForm.isPublic"
          class="mt-4"
          label="公开下载池（被选为 302 重定向目标，需匿名可读）"
        />

        <div v-if="editForm.s3" class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="Bucket" required>
            <UInput v-model="editForm.s3.bucket" class="w-full font-mono" />
          </UFormField>
          <UFormField label="Region" required>
            <UInput v-model="editForm.s3.region" class="w-full font-mono" />
          </UFormField>
          <UFormField label="Access Key" required>
            <UInput v-model="editForm.s3.access_key" class="w-full font-mono" />
          </UFormField>
          <UCheckbox v-model="editForm.s3.path_style" class="md:col-span-2" label="Path Style" />
        </div>

        <div v-if="editForm.ftp" class="mt-4 grid gap-4 md:grid-cols-2">
          <UFormField label="用户名" required>
            <UInput v-model="editForm.ftp.user" class="w-full font-mono" />
          </UFormField>
          <UFormField label="根目录" hint="可选">
            <UInput v-model="editForm.ftp.root" class="w-full font-mono" />
          </UFormField>
          <UFormField label="端口">
            <UInput v-model="editForm.ftp.port" class="w-full font-mono" />
          </UFormField>
        </div>

        <p v-if="editError" class="mt-2 text-xs text-error">{{ editError }}</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="editing = null" />
        <UButton label="保存" @click="submitEdit" />
      </template>
    </UModal>

    <!-- 改密钥 -->
    <UModal
      v-model:open="rotateOpen"
      :title="rotating ? `改密钥：${rotating.id}` : '改密钥'"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <UFormField :label="rotating?.kind === 's3' ? '新 Secret Key' : '新密码'">
          <UInput v-model="rotatedSecret" type="password" class="w-full" autofocus />
        </UFormField>
        <p class="mt-2 text-xs text-dimmed">旧密钥将立即失效；已建立的连接不受影响</p>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" label="取消" @click="rotating = null" />
        <UButton
          label="保存"
          :disabled="rotatedSecret === ''"
          @click="submitRotate"
        />
      </template>
    </UModal>

    <!-- 池内资源 -->
    <UModal
      v-model:open="detailOpen"
      :title="poolDetail ? `池内资源：${poolDetail.id}` : '池内资源'"
    >
      <template #body>
        <div v-if="membersLoading" class="flex justify-center py-10">
          <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-dimmed" />
        </div>
        <UAlert
          v-else-if="membersError"
          color="error"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="membersError"
        />
        <p v-else-if="(members ?? []).length === 0" class="text-sm text-muted">
          池内暂无资源，从下方加入
        </p>
        <div v-else class="space-y-1.5">
          <div
            v-for="member in members ?? []"
            :key="member.sha256"
            class="flex items-center gap-2 rounded-md bg-elevated px-2 py-1.5"
          >
            <span class="max-w-40 truncate font-medium">{{ member.name ?? '—' }}</span>
            <HashChip :hash="member.sha256" :head="10" />
            <span class="ms-auto flex items-center gap-2 text-xs text-muted">
              <span class="tabular-nums">{{ formatBytes(member.size) }}</span>
              <span>{{ formatTime(member.added_at) }}</span>
              <UButton
                size="xs"
                variant="ghost"
                color="error"
                label="移除"
                @click="removeMember(member)"
              />
            </span>
          </div>
        </div>

        <div class="mt-4 flex items-end gap-2 border-t border-default pt-3">
          <UFormField label="加入资源（从本地库选择）" class="grow">
            <USelect
              v-model="selectedMember"
              :items="memberOptions"
              :loading="allResourcesLoading"
              class="w-full"
              placeholder="选择资源"
            />
          </UFormField>
          <UButton label="加入" :disabled="!selectedMember" @click="addMember" />
        </div>
        <p v-if="memberError" class="mt-2 text-xs text-error">{{ memberError }}</p>
      </template>
    </UModal>
  </UCard>
</template>
