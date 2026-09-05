import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { PoolResourceView, PoolView, ResourceView } from '../api/types';
import { useConfirm } from '../components/ConfirmDialog';
import HashChip, { formatBytes, formatTime } from '../components/HashChip';
import Modal from '../components/Modal';
import { Empty, ErrorAlert, Field, Loading } from '../components/States';
import { useAsync } from '../hooks/useAsync';

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

/** 池列表里的参数摘要（s3 → bucket @ region，ftp → user@host:port） */
function SyncCell({ pool }: { pool: PoolView }) {
  const badge = (() => {
    switch (pool.sync_status) {
      case 'synced':
        return { label: '已同步', cls: 'badge-success' };
      case 'syncing':
        return { label: `同步中（${pool.sync_pending}）`, cls: 'badge-warning' };
      case 'error':
        return { label: '同步出错', cls: 'badge-error' };
      default:
        return { label: '未同步', cls: 'badge-ghost' };
    }
  })();

  return (
    <div>
      <span
        className={`badge badge-soft ${badge.cls}`}
        title={pool.sync_error ?? undefined}
      >
        {badge.label}
      </span>
      <div className="mt-0.5 text-xs opacity-60">
        {pool.object_count} 项 · {formatBytes(pool.total_size)}
      </div>
    </div>
  );
}

function poolSummary(pool: PoolView): string {
  if (pool.kind === 's3') {
    const config = pool.config as Partial<S3Form>;
    return [config.bucket, config.region].filter(Boolean).join(' @ ') || '—';
  }
  const config = pool.config as Partial<FtpForm>;
  const host = pool.endpoint.replace(/^https?:\/\//, '');
  return `${config.user || '?'}@${host}${config.port ? `:${config.port}` : ''}`;
}

export default function PoolsPage({ client }: { client: ApiClient }) {
  const pools = useAsync(() => client.listPools(), [client]);
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<PoolView | null>(null);
  const [rotating, setRotating] = useState<PoolView | null>(null);
  const [poolDetail, setPoolDetail] = useState<PoolView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [confirmEl, askConfirm] = useConfirm();

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      pools.reload();
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    }
  }

  return (
    <section className="card border border-base-300 bg-base-100">
      <div className="card-body gap-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="card-title">资源池</h2>
            <p className="text-base-content/60 text-sm">
              仅保存配置；密钥不回显，仅可重设；公开池参与下载重定向
            </p>
          </div>
          <div className="flex gap-2">
            <button className="btn btn-sm" onClick={pools.reload} disabled={pools.loading}>
              刷新
            </button>
            <button
              className="btn btn-sm"
              disabled={scanning}
              onClick={async () => {
                setScanning(true);
                setError(null);
                try {
                  await client.scanPools();
                  pools.reload();
                } catch (e) {
                  setError(e instanceof Error ? e.message : String(e));
                } finally {
                  setScanning(false);
                }
              }}
            >
              {scanning ? '扫描中…' : '强制扫描'}
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setCreating(true)}>
              新建资源池
            </button>
          </div>
        </div>

        {error && <ErrorAlert message={error} />}

        {pools.loading && <Loading />}
        {pools.data?.length === 0 && <Empty>暂无资源池，请点击右上角「新建资源池」</Empty>}

        {pools.data && pools.data.length > 0 && (
          <div className="overflow-x-auto">
            <table className="table">
              <thead>
                <tr>
                  <th>Id</th>
                  <th>类型</th>
                  <th>终结点</th>
                  <th>参数</th>
                  <th>健康</th>
                  <th>同步</th>
                  <th>公开下载</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {pools.data.map((pool) => (
                  <tr key={pool.id} className="hover">
                    <td className="font-mono text-xs">{pool.id}</td>
                    <td>
                      <span className="badge badge-soft badge-outline">{pool.kind}</span>
                    </td>
                    <td className="max-w-56 truncate font-mono text-xs opacity-70">
                      {pool.endpoint}
                    </td>
                    <td className="max-w-48 truncate font-mono text-xs opacity-70">
                      {poolSummary(pool)}
                    </td>
                    <td>
                      {pool.connected ? (
                        pool.up ? (
                          <span
                            className="badge badge-success badge-soft"
                            title={`${pool.health_checked_at ?? '—'} · 探测通过`}
                          >
                            健康
                          </span>
                        ) : (
                          <span
                            className="badge badge-error badge-soft"
                            title={pool.health_error ?? '探测失败'}
                          >
                            异常
                          </span>
                        )
                      ) : (
                        <span className="badge badge-ghost badge-soft">未连接</span>
                      )}
                    </td>
                    <td>
                      <SyncCell pool={pool} />
                    </td>
                    <td>
                      <label className="flex cursor-pointer items-center gap-2">
                        <input
                          type="checkbox"
                          className="toggle toggle-primary toggle-sm"
                          checked={pool.is_public}
                          onChange={() =>
                            void run(() => client.updatePool(pool.id, { is_public: !pool.is_public }))
                          }
                        />
                        <span className="text-xs">{pool.is_public ? '公开' : '内部'}</span>
                      </label>
                    </td>
                    <td className="text-right">
                      <details className="dropdown dropdown-end">
                        <summary className="btn btn-xs">操作</summary>
                        <ul className="dropdown-content menu z-10 w-40 rounded-box border border-base-300 bg-base-100 p-1 shadow">
                          <li>
                            <button onClick={() => setPoolDetail(pool)}>池内资源</button>
                          </li>
                          <li>
                            <button onClick={() => setEditing(pool)}>编辑</button>
                          </li>
                          <li>
                            <button onClick={() => setRotating(pool)}>改密钥</button>
                          </li>
                          <li>
                            <button
                              className="text-error"
                              onClick={() => {
                                void askConfirm(`确认删除资源池「${pool.id}」？`).then((ok) => {
                                  if (ok) void run(() => client.deletePool(pool.id));
                                });
                              }}
                            >
                              删除
                            </button>
                          </li>
                        </ul>
                      </details>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <CreatePoolModal
        open={creating}
        onClose={() => setCreating(false)}
        onCreate={async (body) => {
          if (await run(() => client.createPool(body))) setCreating(false);
        }}
      />

      <EditPoolModal
        pool={editing}
        onClose={() => setEditing(null)}
        onSave={async (id, body) => {
          if (await run(() => client.updatePool(id, body))) setEditing(null);
        }}
      />

      <RotateSecretModal
        pool={rotating}
        onClose={() => setRotating(null)}
        onRotate={async (id, secret) => {
          if (await run(() => client.updatePool(id, { secret }))) setRotating(null);
        }}
      />

      <PoolResourcesModal client={client} pool={poolDetail} onClose={() => setPoolDetail(null)} />

      {confirmEl}
    </section>
  );
}

// ---------- 表单公共件 ----------

function CommonFields({
  form,
  setForm,
}: {
  form: { endpoint: string; public_endpoint: string; is_public: boolean };
  setForm: (patch: Partial<typeof form>) => void;
}) {
  return (
    <>
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="终结点" required>
          <input
            className="input w-full font-mono text-xs"
            placeholder="s3: http://minio:9000 / ftp: ftp.example.com"
            value={form.endpoint}
            onChange={(e) => setForm({ endpoint: e.target.value })}
          />
        </Field>
        <Field label="公开终结点">
          <input
            className="input w-full font-mono text-xs"
            placeholder="https://dl.example.com"
            value={form.public_endpoint}
            onChange={(e) => setForm({ public_endpoint: e.target.value })}
          />
        </Field>
      </div>
      <label className="option-row">
        <input
          type="checkbox"
          className="checkbox checkbox-primary checkbox-sm"
          checked={form.is_public}
          onChange={(e) => setForm({ is_public: e.target.checked })}
        />
        <span>公开下载池（被选为 302 重定向目标，需匿名可读）</span>
      </label>
    </>
  );
}

/** 按类型组装提交用的 config / secret */
function buildCreateBody(kind: string, form: CreateForm) {
  return {
    id: form.id.trim(),
    kind,
    endpoint: form.endpoint.trim(),
    public_endpoint: form.public_endpoint.trim(),
    is_public: form.is_public,
    secret: kind === 's3' ? form.s3.secret : form.ftp.password,
    config:
      kind === 's3'
        ? { bucket: form.s3.bucket.trim(), region: form.s3.region.trim(), access_key: form.s3.access_key.trim(), path_style: form.s3.path_style }
        : { user: form.ftp.user.trim(), root: form.ftp.root.trim() || undefined, port: form.ftp.port.trim() ? Number(form.ftp.port) : undefined },
  };
}

interface CreateForm {
  id: string;
  endpoint: string;
  public_endpoint: string;
  is_public: boolean;
  s3: S3Form;
  ftp: FtpForm;
}

function CreatePoolModal({
  open,
  onClose,
  onCreate,
}: {
  open: boolean;
  onClose: () => void;
  onCreate: (body: ReturnType<typeof buildCreateBody>) => Promise<void>;
}) {
  const [kind, setKind] = useState<'s3' | 'ftp'>('s3');
  const [form, setForm] = useState<CreateForm>({
    id: '',
    endpoint: '',
    public_endpoint: '',
    is_public: false,
    s3: { bucket: '', region: '', access_key: '', secret: '', path_style: true },
    ftp: { user: '', password: '', root: '', port: '21' },
  });
  const [error, setError] = useState<string | null>(null);

  const setCommon = (patch: Partial<CreateForm>) => setForm((f) => ({ ...f, ...patch }));

  function validate(): string | null {
    if (!form.id.trim()) return 'Id 必填';
    if (kind === 's3') {
      if (!form.s3.bucket.trim() || !form.s3.region.trim() || !form.s3.access_key.trim() || !form.s3.secret) {
        return 'S3 需要 bucket、region、access key 和 secret key';
      }
    } else if (!form.ftp.user.trim() || !form.ftp.password) {
      return 'FTP 需要用户名和密码';
    }
    if (form.is_public && !form.public_endpoint.trim()) return '公开下载需要填写公开终结点';
    if (form.ftp.port.trim() && Number.isNaN(Number(form.ftp.port))) return 'FTP 端口必须是数字';
    return null;
  }

  return (
    <Modal
      open={open}
      title="新建资源池"
      onClose={onClose}
      footer={
        <button
          className="btn btn-primary"
          onClick={() => {
            const message = validate();
            if (message) {
              setError(message);
              return;
            }
            setError(null);
            void onCreate(buildCreateBody(kind, form)).then(() =>
              setForm({
                id: '',
                endpoint: '',
                public_endpoint: '',
                is_public: false,
                s3: { bucket: '', region: '', access_key: '', secret: '', path_style: true },
                ftp: { user: '', password: '', root: '', port: '21' },
              }),
            );
          }}
        >
          创建
        </button>
      }
    >
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="Id" required>
          <input
            className="input w-full font-mono"
            value={form.id}
            onChange={(e) => setForm({ ...form, id: e.target.value })}
          />
        </Field>
        <Field label="类型">
          <select
            className="select w-full"
            value={kind}
            onChange={(e) => setKind(e.target.value as 's3' | 'ftp')}
          >
            <option value="s3">s3</option>
            <option value="ftp">ftp</option>
          </select>
        </Field>
      </div>

      <CommonFields form={form} setForm={setCommon} />

      {/* 字段组随类型切换 */}
      {kind === 's3' ? (
        <div className="grid gap-3 md:grid-cols-2">
          <Field label="Bucket" required>
            <input
              className="input w-full font-mono text-xs"
              placeholder="coo"
              value={form.s3.bucket}
              onChange={(e) => setForm({ ...form, s3: { ...form.s3, bucket: e.target.value } })}
            />
          </Field>
          <Field label="Region" required>
            <input
              className="input w-full font-mono text-xs"
              placeholder="us-east-1"
              value={form.s3.region}
              onChange={(e) => setForm({ ...form, s3: { ...form.s3, region: e.target.value } })}
            />
          </Field>
          <Field label="Access Key" required>
            <input
              className="input w-full font-mono text-xs"
              placeholder="access id"
              value={form.s3.access_key}
              onChange={(e) => setForm({ ...form, s3: { ...form.s3, access_key: e.target.value } })}
            />
          </Field>
          <Field label="Secret Key" required>
            <input
              type="password"
              className="input w-full font-mono text-xs"
              placeholder="access token"
              value={form.s3.secret}
              onChange={(e) => setForm({ ...form, s3: { ...form.s3, secret: e.target.value } })}
            />
          </Field>
        </div>
      ) : (
        <div className="grid gap-3 md:grid-cols-2">
          <Field label="用户名" required>
            <input
              className="input w-full font-mono text-xs"
              placeholder="ftp user"
              value={form.ftp.user}
              onChange={(e) => setForm({ ...form, ftp: { ...form.ftp, user: e.target.value } })}
            />
          </Field>
          <Field label="密码" required>
            <input
              type="password"
              className="input w-full font-mono text-xs"
              value={form.ftp.password}
              onChange={(e) => setForm({ ...form, ftp: { ...form.ftp, password: e.target.value } })}
            />
          </Field>
          <Field label="根目录（可选）">
            <input
              className="input w-full font-mono text-xs"
              placeholder="/coo"
              value={form.ftp.root}
              onChange={(e) => setForm({ ...form, ftp: { ...form.ftp, root: e.target.value } })}
            />
          </Field>
          <Field label="端口">
            <input
              className="input w-full font-mono text-xs"
              value={form.ftp.port}
              onChange={(e) => setForm({ ...form, ftp: { ...form.ftp, port: e.target.value } })}
            />
          </Field>
        </div>
      )}

      <label className="option-row">
        <input
          type="checkbox"
          className="checkbox checkbox-primary checkbox-sm"
          checked={form.s3.path_style}
          onChange={(e) => setForm({ ...form, s3: { ...form.s3, path_style: e.target.checked } })}
        />
        <span>
          Path Style
          <span className="text-xs opacity-60">（默认开启，适配 MinIO 等自托管对象存储）</span>
        </span>
      </label>
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}

function EditPoolModal({
  pool,
  onClose,
  onSave,
}: {
  pool: PoolView | null;
  onClose: () => void;
  onSave: (id: string, body: Record<string, unknown>) => Promise<void>;
}) {
  const [endpoint, setEndpoint] = useState(pool?.endpoint ?? '');
  const [publicEndpoint, setPublicEndpoint] = useState(pool?.public_endpoint ?? '');
  const [isPublic, setIsPublic] = useState(pool?.is_public ?? false);
  const [s3, setS3] = useState<S3Form | null>(
    pool?.kind === 's3'
      ? {
          bucket: (pool.config.bucket as string) ?? '',
          region: (pool.config.region as string) ?? '',
          access_key: (pool.config.access_key as string) ?? '',
          secret: '',
          path_style: (pool.config.path_style as boolean) ?? true,
        }
      : null,
  );
  const [ftp, setFtp] = useState<FtpForm | null>(
    pool?.kind === 'ftp'
      ? {
          user: (pool.config.user as string) ?? '',
          password: '',
          root: (pool.config.root as string) ?? '',
          port: String((pool.config.port as number) ?? 21),
        }
      : null,
  );
  const [error, setError] = useState<string | null>(null);

  if (!pool) return null;
  const target = pool;
  const kind = target.kind;

  function currentConfig(): Record<string, unknown> {
    return kind === 's3' && s3
      ? { bucket: s3.bucket.trim(), region: s3.region.trim(), access_key: s3.access_key.trim(), path_style: s3.path_style }
      : ftp
        ? { user: ftp.user.trim(), root: ftp.root.trim() || undefined, port: ftp.port.trim() ? Number(ftp.port) : undefined }
        : {};
  }

  const configChanged =
    JSON.stringify(currentConfig(), Object.keys(currentConfig()).sort()) !==
    JSON.stringify(target.config, Object.keys(target.config).sort());

  function submit() {
    if (kind === 's3' && (!s3?.bucket.trim() || !s3.region.trim() || !s3.access_key.trim())) {
      setError('S3 需要 bucket、region、access key');
      return;
    }
    if (kind === 'ftp' && !ftp?.user.trim()) {
      setError('FTP 需要用户名');
      return;
    }
    if (isPublic && !publicEndpoint.trim()) {
      setError('公开下载需要填写公开终结点');
      return;
    }

    const body: Record<string, unknown> = {};
    if (endpoint !== target.endpoint) body.endpoint = endpoint.trim();
    if (publicEndpoint !== target.public_endpoint) body.public_endpoint = publicEndpoint.trim();
    if (isPublic !== target.is_public) body.is_public = isPublic;
    if (configChanged) body.config = currentConfig();

    if (Object.keys(body).length === 0) {
      onClose();
      return;
    }
    setError(null);
    void onSave(target.id, body);
  }

  return (
    <Modal
      open
      title={`编辑资源池：${pool.id}`}
      onClose={onClose}
      footer={
        <button className="btn" onClick={submit}>
          保存
        </button>
      }
    >
      <div className="mb-2 flex items-center gap-2">
        <span className="badge badge-soft badge-outline">{pool.kind}</span>
        <span className="text-xs opacity-60">类型不可变更；密钥修改请使用列表中的「改密钥」</span>
      </div>

      <CommonFields
        form={{ endpoint, public_endpoint: publicEndpoint, is_public: isPublic }}
        setForm={(patch) => {
          if (patch.endpoint !== undefined) setEndpoint(patch.endpoint);
          if (patch.public_endpoint !== undefined) setPublicEndpoint(patch.public_endpoint);
          if (patch.is_public !== undefined) setIsPublic(patch.is_public);
        }}
      />

      {pool.kind === 's3' && s3 && (
        <div className="grid gap-3 md:grid-cols-2">
          <Field label="Bucket" required>
            <input
              className="input w-full font-mono text-xs"
              value={s3.bucket}
              onChange={(e) => setS3({ ...s3, bucket: e.target.value })}
            />
          </Field>
          <Field label="Region" required>
            <input
              className="input w-full font-mono text-xs"
              value={s3.region}
              onChange={(e) => setS3({ ...s3, region: e.target.value })}
            />
          </Field>
          <Field label="Access Key" required>
            <input
              className="input w-full font-mono text-xs"
              value={s3.access_key}
              onChange={(e) => setS3({ ...s3, access_key: e.target.value })}
            />
          </Field>
        </div>
      )}
      {pool.kind === 's3' && s3 && (
        <label className="option-row">
          <input
            type="checkbox"
            className="checkbox checkbox-primary checkbox-sm"
            checked={s3.path_style}
            onChange={(e) => setS3({ ...s3, path_style: e.target.checked })}
          />
          <span>Path Style</span>
        </label>
      )}

      {pool.kind === 'ftp' && ftp && (
        <div className="grid gap-3 md:grid-cols-2">
          <Field label="用户名" required>
            <input
              className="input w-full font-mono text-xs"
              value={ftp.user}
              onChange={(e) => setFtp({ ...ftp, user: e.target.value })}
            />
          </Field>
          <Field label="根目录（可选）">
            <input
              className="input w-full font-mono text-xs"
              value={ftp.root}
              onChange={(e) => setFtp({ ...ftp, root: e.target.value })}
            />
          </Field>
          <Field label="端口">
            <input
              className="input w-full font-mono text-xs"
              value={ftp.port}
              onChange={(e) => setFtp({ ...ftp, port: e.target.value })}
            />
          </Field>
        </div>
      )}
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}

function RotateSecretModal({
  pool,
  onClose,
  onRotate,
}: {
  pool: PoolView | null;
  onClose: () => void;
  onRotate: (id: string, secret: string) => Promise<void>;
}) {
  const [secret, setSecret] = useState('');

  if (!pool) return null;

  return (
    <Modal
      open
      title={`改密钥：${pool.id}`}
      onClose={onClose}
      footer={
        <button
          className="btn"
          disabled={secret === ''}
          onClick={() => void onRotate(pool.id, secret)}
        >
          保存
        </button>
      }
    >
      <Field label={pool.kind === 's3' ? '新 Secret Key' : '新密码'}>
        <input
          type="password"
          className="input w-full"
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          autoFocus
        />
      </Field>
      <p className="text-base-content/50 text-xs">旧密钥将立即失效；已建立的连接不受影响</p>
    </Modal>
  );
}

function PoolResourcesModal({
  client,
  pool,
  onClose,
}: {
  client: ApiClient;
  pool: PoolView | null;
  onClose: () => void;
}) {
  const members = useAsync(
    () => (pool ? client.poolResources(pool.id) : Promise.resolve<PoolResourceView[]>([])),
    [client, pool],
  );
  const all = useAsync(
    () => (pool ? client.listResources() : Promise.resolve<ResourceView[]>([])),
    [client, pool],
  );
  const [selected, setSelected] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [confirmEl, askConfirm] = useConfirm();

  if (!pool) return null;
  const target = pool;

  const candidates = (all.data ?? []).filter(
    (resource) => !(members.data ?? []).some((member) => member.sha256 === resource.sha256),
  );

  async function add() {
    if (!selected) return;
    setError(null);
    try {
      await client.poolAddResource(target.id, selected);
      setSelected('');
      members.reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <Modal open title={`池内资源：${pool.id}`} onClose={onClose}>
      {members.loading && <Loading />}
      {members.error && <ErrorAlert message={members.error} />}
      {!members.loading && members.data?.length === 0 && (
        <p className="text-base-content/60 text-sm">池内暂无资源，从下方加入</p>
      )}
      {members.data && members.data.length > 0 && (
        <div className="space-y-1.5">
          {members.data.map((member) => (
            <div
              key={member.sha256}
              className="flex items-center gap-2 border border-base-300 bg-base-200 px-2 py-1.5"
            >
              <span className="max-w-40 truncate font-medium">{member.name ?? '—'}</span>
              <HashChip hash={member.sha256} head={10} />
              <span className="ml-auto flex items-center gap-2 text-xs opacity-60">
                <span className="tabular-nums">{formatBytes(member.size)}</span>
                <span>{formatTime(member.added_at)}</span>
                <button
                  className="btn btn-xs btn-ghost text-error"
                  onClick={() => {
                    void askConfirm('确认从该池移除？本地副本与登记不受影响。').then((ok) => {
                      if (ok) {
                        void client
                          .poolRemoveResource(pool.id, member.sha256)
                          .then(() => members.reload())
                          .catch((e) => setError(e instanceof Error ? e.message : String(e)));
                      }
                    });
                  }}
                >
                  移除
                </button>
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="mt-4 flex items-end gap-2 border-t border-base-300 pt-3">
        <label className="form-control grow">
          <div className="label py-1">
            <span className="label-text">加入资源（从本地库选择）</span>
          </div>
          <select
            className="select w-full"
            value={selected}
            onChange={(e) => setSelected(e.target.value)}
          >
            <option value="">选择资源</option>
            {candidates.map((resource) => (
              <option key={resource.sha256} value={resource.sha256}>
                {resource.name ?? resource.sha256.slice(0, 12)} · {resource.sha256.slice(0, 12)}
              </option>
            ))}
          </select>
        </label>
        <button className="btn btn-primary" disabled={!selected} onClick={() => void add()}>
          加入
        </button>
      </div>
      {error && <p className="text-error mt-2 text-xs">{error}</p>}
      {confirmEl}
    </Modal>
  );
}

