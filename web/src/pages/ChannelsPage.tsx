import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { AdminChannelView, UpdateView } from '../api/types';
import { useConfirm } from '../components/ConfirmDialog';
import HashChip, { formatBytes, formatTime, isSha256 } from '../components/HashChip';
import Modal from '../components/Modal';
import ResourcePicker from '../components/ResourcePicker';
import { Empty, ErrorAlert, Field, Loading } from '../components/States';
import { useAsync } from '../hooks/useAsync';

/** 发版通道：应用内管理。行展开看版本记录与差分。 */
export default function ChannelsPage({ client, appId }: { client: ApiClient; appId: string }) {
  const channels = useAsync(
    () => client.listChannelsAdmin(appId),
    [client, appId],
  );
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<AdminChannelView | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmEl, askConfirm] = useConfirm();

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      channels.reload();
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
            <h2 className="card-title">发版通道</h2>
            <p className="text-base-content/60 text-sm">本应用的通道、版本记录与差分</p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <button className="btn btn-sm" onClick={channels.reload} disabled={channels.loading}>
              刷新
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setCreating(true)}>
              新建通道
            </button>
          </div>
        </div>

        {error && <ErrorAlert message={error} />}

        {channels.loading && <Loading />}
        {!channels.loading && channels.data?.length === 0 && (
          <Empty>暂无发版通道；请先上传资源，再创建通道</Empty>
        )}

        {channels.data && channels.data.length > 0 && (
          <div className="overflow-x-auto">
            <table className="table">
              <thead>
                <tr>
                  <th>tag</th>
                  <th>最新版本</th>
                  <th>完整包指纹</th>
                  <th className="text-right">大小</th>
                  <th>默认</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {channels.data.map((channel) => (
                  <>
                    <tr key={channel.guid} className="hover">
                      <td className="font-medium">{channel.tag_name}</td>
                      <td className="font-mono text-xs">{channel.latest_version}</td>
                      <td>
                        <HashChip hash={channel.raw_sha256} />
                      </td>
                      <td className="text-right tabular-nums text-sm">
                        {formatBytes(channel.raw_size)}
                      </td>
                      <td>
                        {channel.is_default ? (
                          <span className="badge badge-primary badge-soft">默认</span>
                        ) : (
                          <span className="opacity-40">—</span>
                        )}
                      </td>
                      <td className="text-right">
                        <div className="join">
                          <button
                            className="btn btn-xs join-item"
                            onClick={() =>
                              setExpanded(expanded === channel.guid ? null : channel.guid)
                            }
                          >
                            {expanded === channel.guid ? '收起' : '版本 / 差分'}
                          </button>
                          <button className="btn btn-xs join-item" onClick={() => setEditing(channel)}>
                            编辑
                          </button>
                          <button
                            className="btn btn-xs btn-error join-item"
                            onClick={() => {
                              void askConfirm(
                                `确认删除通道「${channel.tag_name}」？其版本记录与差分将一并删除。`,
                              ).then((ok) => {
                                if (ok) void run(() => client.deleteChannel(channel.guid));
                              });
                            }}
                          >
                            删除
                          </button>
                        </div>
                      </td>
                    </tr>
                    {expanded === channel.guid && (
                      <tr key={`${channel.guid}-detail`} className="bg-base-200/60">
                        <td colSpan={6} className="p-0">
                          <ChannelDetail client={client} guid={channel.guid} />
                        </td>
                      </tr>
                    )}
                  </>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <CreateChannelModal
          client={client}
          appId={appId}
          open={creating}
          onClose={() => setCreating(false)}
          onCreate={async (body) => {
            if (await run(() => client.createChannel(body))) setCreating(false);
          }}
        />

      <EditChannelModal
        client={client}
        channel={editing}
        onClose={() => setEditing(null)}
        onSave={async (guid, body) => {
          if (await run(() => client.updateChannel(guid, body))) setEditing(null);
        }}
      />

      {confirmEl}
    </section>
  );
}

/** 展开块：版本记录 + 差分 + 客户端视图预览 */
function ChannelDetail({ client, guid }: { client: ApiClient; guid: string }) {
  const releases = useAsync(() => client.listReleases(guid), [client, guid]);
  const diffs = useAsync(() => client.listDiffs(guid), [client, guid]);
  const [version, setVersion] = useState('');
  const [releaseSha, setReleaseSha] = useState('');
  const [baseSha, setBaseSha] = useState('');
  const [patchSha, setPatchSha] = useState('');
  const [algo, setAlgo] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState(false);
  const [confirmEl, askConfirm] = useConfirm();

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      releases.reload();
      diffs.reload();
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    }
  }

  function addRelease() {
    if (!version.trim() || !isSha256(releaseSha)) {
      setError('版本必填，sha256 需为 64 位十六进制');
      return;
    }
    void run(() =>
      client.createRelease(guid, { version: version.trim(), sha256: releaseSha }),
    ).then((ok) => {
      if (ok) {
        setVersion('');
        setReleaseSha('');
      }
    });
  }

  function addDiff() {
    if (!isSha256(baseSha) || !isSha256(patchSha)) {
      setError('base / patch sha256 需为 64 位十六进制');
      return;
    }
    if (baseSha === patchSha) {
      setError('base 与 patch 不能相同');
      return;
    }
    void run(() =>
      client.createDiff(guid, { base_sha256: baseSha, patch_sha256: patchSha, algo: algo || null }),
    ).then((ok) => {
      if (ok) {
        setBaseSha('');
        setPatchSha('');
        setAlgo('');
      }
    });
  }

  return (
    <div className="grid gap-6 p-4 sm:grid-cols-2">
      <div className="min-w-0">
        <div className="mb-2 flex items-baseline justify-between">
          <span className="eyebrow">版本记录 · Releases</span>
          <button className="text-primary btn-link btn-xs" onClick={() => setPreview(true)}>
            客户端视图预览
          </button>
        </div>
        <div className="space-y-2">
          {releases.loading && <Loading />}
          {releases.error && <ErrorAlert message={releases.error} />}
          {!releases.loading && releases.data?.length === 0 && (
            <p className="text-base-content/60 text-sm">暂无版本记录</p>
          )}
          {releases.data?.map((release) => (
            <div
              key={release.version}
              className="flex items-center justify-between gap-2 border border-base-300 bg-base-200 px-2 py-1.5"
            >
              <div className="flex min-w-0 items-center gap-2">
                <span className="font-mono text-sm font-semibold">{release.version}</span>
                <HashChip hash={release.sha256} head={8} />
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs opacity-50">{formatTime(release.created_at)}</span>
                <button
                  className="btn btn-xs btn-ghost text-error"
                  onClick={() =>
                    void askConfirm(`确认删除版本记录 ${release.version}？`).then((ok) => {
                      if (ok) void run(() => client.deleteRelease(guid, release.version));
                    })
                  }
                >
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
        <div className="mt-3 grid grid-cols-[8rem_1fr] gap-2">
          <input
            className="input input-sm font-mono"
            placeholder="* 版本号（如 1.0.0）"
            value={version}
            onChange={(e) => setVersion(e.target.value)}
          />
          <input
            className="input input-sm font-mono text-xs"
            placeholder="* sha256（需为已上传资源）"
            value={releaseSha}
            onChange={(e) => setReleaseSha(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && addRelease()}
          />
        </div>
        <button className="btn btn-sm mt-2" onClick={addRelease}>
          添加版本
        </button>
        {error && <ErrorAlert message={error} />}
      </div>

      <div className="min-w-0">
        <span className="eyebrow mb-2 block">差分 · Diffs</span>
        <div className="space-y-2">
          {diffs.loading && <Loading />}
          {diffs.error && <ErrorAlert message={diffs.error} />}
          {!diffs.loading && diffs.data?.length === 0 && (
            <p className="text-base-content/60 text-sm">暂无差分，客户端将直接下载完整包</p>
          )}
          {diffs.data?.map((diff) => (
            <div key={diff.base_sha256} className="flex items-center gap-2 border border-base-300 bg-base-200 px-2 py-1.5">
              <HashChip hash={diff.base_sha256} head={8} />
              <span className="text-primary">→</span>
              <HashChip hash={diff.patch_sha256} head={8} />
              <span className="ml-auto flex items-center gap-2 text-xs opacity-60">
                <span>{diff.algo ?? '—'}</span>
                <span className="tabular-nums">{formatBytes(diff.size)}</span>
                <button
                  className="btn btn-xs btn-ghost text-error"
                  onClick={() =>
                    void askConfirm('确认删除这条差分？').then((ok) => {
                      if (ok) void run(() => client.deleteDiff(guid, diff.base_sha256));
                    })
                  }
                >
                  删除
                </button>
              </span>
            </div>
          ))}
        </div>
        <div className="mt-3 grid grid-cols-2 gap-2">
          <input
            className="input input-sm font-mono text-xs"
            placeholder="* base sha256"
            value={baseSha}
            onChange={(e) => setBaseSha(e.target.value)}
          />
          <input
            className="input input-sm font-mono text-xs"
            placeholder="* patch sha256"
            value={patchSha}
            onChange={(e) => setPatchSha(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && addDiff()}
          />
        </div>
        <div className="mt-2 flex gap-2">
          <input
            className="input input-sm font-mono text-xs"
            placeholder="算法（可选，如 xdelta3）"
            value={algo}
            onChange={(e) => setAlgo(e.target.value)}
          />
          <button className="btn btn-sm" onClick={addDiff}>
            添加差分
          </button>
        </div>
      </div>

      <PreviewModal client={client} guid={guid} open={preview} onClose={() => setPreview(false)} />

      {confirmEl}
    </div>
  );
}

/** 客户端视角：同一个通道，普通客户端通过 X-App-Id 拿到的响应 */
function PreviewModal({
  client,
  guid,
  open,
  onClose,
}: {
  client: ApiClient;
  guid: string;
  open: boolean;
  onClose: () => void;
}) {
  const update = useAsync(
    () => (open ? client.getUpdate(guid) : Promise.resolve<UpdateView | null>(null)),
    [client, guid, open],
  );
  const data: UpdateView | null = update.data ?? null;

  return (
    <Modal open={open} title="客户端视图预览" onClose={onClose}>
      {update.loading && <Loading />}
      {update.error && <ErrorAlert message={update.error} />}
      {data && (
        <div className="space-y-3">
          <div className="flex items-baseline gap-2">
            <span className="text-sm opacity-60">{data.tag_name}</span>
            <span className="font-mono text-lg font-bold">{data.version}</span>
          </div>
          <div className="flex items-center gap-2 text-sm">
            <span className="opacity-60">完整包</span>
            <HashChip hash={data.raw_sha256} />
            <span className="tabular-nums opacity-60">{formatBytes(data.raw_size)}</span>
          </div>
          <div>
            <span className="eyebrow block py-1">客户端以本地 sha256 比对并选择差分</span>
            {data.diffs.length === 0 ? (
              <p className="text-base-content/60 text-sm">暂无差分，客户端将直接下载完整包</p>
            ) : (
              <div className="space-y-1.5">
                {data.diffs.map((diff) => (
                  <div key={diff.base_sha256} className="flex items-center gap-2">
                    <HashChip hash={diff.base_sha256} head={8} />
                    <span className="text-primary">→</span>
                    <HashChip hash={diff.patch_sha256} head={8} />
                    <span className="ml-auto text-xs opacity-60">{diff.algo ?? '—'}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}

function CreateChannelModal({
  client,
  appId,
  open,
  onClose,
  onCreate,
}: {
  client: ApiClient;
  appId: string;
  open: boolean;
  onClose: () => void;
  onCreate: (body: {
    app_id: string;
    tag_name: string;
    latest_version: string;
    latest_sha256: string;
    is_default?: boolean;
  }) => Promise<void>;
}) {
  const [form, setForm] = useState({
    tag_name: '',
    latest_version: '',
    latest_sha256: '',
    is_default: false,
  });
  const [error, setError] = useState<string | null>(null);

  function submit() {
    if (!form.tag_name.trim() || !form.latest_version.trim()) {
      setError('tag、版本必填');
      return;
    }
    if (!form.latest_sha256) {
      setError('请选择完整包资源');
      return;
    }
    setError(null);
    void onCreate({
      app_id: appId,
      tag_name: form.tag_name.trim(),
      latest_version: form.latest_version.trim(),
      latest_sha256: form.latest_sha256.toLowerCase(),
      is_default: form.is_default,
    });
  }

  return (
    <Modal
      open={open}
      title="新建通道"
      onClose={onClose}
      footer={
        <button className="btn" onClick={submit}>
          创建
        </button>
      }
    >

      <div className="grid gap-3 md:grid-cols-2">
        <Field label="tag 名" required>
          <input
            className="input w-full font-mono"
            placeholder="stable"
            value={form.tag_name}
            onChange={(e) => setForm({ ...form, tag_name: e.target.value })}
          />
        </Field>
        <Field label="最新版本" required>
          <input
            className="input w-full font-mono"
            placeholder="1.1.0"
            value={form.latest_version}
            onChange={(e) => setForm({ ...form, latest_version: e.target.value })}
          />
        </Field>
      </div>
      <Field label="完整包资源（必须先上传）" required>
        <ResourcePicker
          client={client}
          value={form.latest_sha256}
          onChange={(sha256) => setForm({ ...form, latest_sha256: sha256 ?? '' })}
        />
      </Field>
      <label className="option-row">
        <input
          type="checkbox"
          className="checkbox checkbox-primary checkbox-sm"
          checked={form.is_default}
          onChange={(e) => setForm({ ...form, is_default: e.target.checked })}
        />
        <span>设为默认通道（同应用其他默认将自动取消）</span>
      </label>
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}

function EditChannelModal({
  client,
  channel,
  onClose,
  onSave,
}: {
  client: ApiClient;
  channel: AdminChannelView | null;
  onClose: () => void;
  onSave: (guid: string, body: Record<string, unknown>) => Promise<void>;
}) {
  const [tag, setTag] = useState(channel?.tag_name ?? '');
  const [version, setVersion] = useState(channel?.latest_version ?? '');
  const [sha, setSha] = useState<string | null>(channel?.raw_sha256 ?? null);
  const [isDefault, setIsDefault] = useState(channel?.is_default ?? false);
  const [error, setError] = useState<string | null>(null);

  if (!channel) return null;
  const target = channel;

  function submit() {
    const body: Record<string, unknown> = {};
    if (tag.trim() && tag.trim() !== target.tag_name) body.tag_name = tag.trim();
    if (version.trim() && version.trim() !== target.latest_version)
      body.latest_version = version.trim();
    if (sha && sha.toLowerCase() !== target.raw_sha256.toLowerCase()) {
      body.latest_sha256 = sha.toLowerCase();
    }
    if (isDefault !== target.is_default) body.is_default = isDefault;

    if (Object.keys(body).length === 0) {
      onClose();
      return;
    }
    setError(null);
    void onSave(target.guid, body);
  }

  return (
    <Modal
      open
      title={`编辑通道：${target.tag_name}`}
      onClose={onClose}
      footer={
        <button className="btn" onClick={submit}>
          保存
        </button>
      }
    >
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="tag 名" required>
          <input
            className="input w-full font-mono"
            value={tag}
            onChange={(e) => setTag(e.target.value)}
          />
        </Field>
        <Field label="最新版本" required>
          <input
            className="input w-full font-mono"
            value={version}
            onChange={(e) => setVersion(e.target.value)}
          />
        </Field>
      </div>
      <Field label="完整包资源">
        <ResourcePicker client={client} value={sha} onChange={setSha} />
      </Field>
      <label className="option-row">
        <input
          type="checkbox"
          className="checkbox checkbox-primary checkbox-sm"
          checked={isDefault}
          onChange={(e) => setIsDefault(e.target.checked)}
        />
        <span>设为默认（同应用其他通道将自动取消）</span>
      </label>
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}