import { useRef, useState } from 'react';
import type { ApiClient } from '../api/client';
import type { ResourceDetailView } from '../api/types';
import { useConfirm } from '../components/ConfirmDialog';
import HashChip, { formatBytes, formatTime } from '../components/HashChip';
import Modal from '../components/Modal';
import { Empty, ErrorAlert, Field, Loading } from '../components/States';
import { useAsync } from '../hooks/useAsync';

interface SelectedFile {
  file: File;
  key: string;
}

/** 资源库：内容寻址、永远先落本地；进池/同步是资源池管理的事。 */
export default function ResourcesPage({ client }: { client: ApiClient }) {
  const resources = useAsync(() => client.listResources(), [client]);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [detailSha, setDetailSha] = useState<string | null>(null);
  const [confirmEl, askConfirm] = useConfirm();

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      resources.reload();
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
            <h2 className="card-title">资源</h2>
            <p className="text-base-content/60 text-sm">
              本地上传（内容寻址）；进入资源池与同步由「资源池」页负责
            </p>
          </div>
          <div className="flex gap-2">
            <button className="btn btn-sm" onClick={resources.reload} disabled={resources.loading}>
              刷新
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setUploadOpen(true)}>
              上传资源
            </button>
          </div>
        </div>

        {error && <ErrorAlert message={error} />}

        {resources.loading && <Loading />}
        {!resources.loading && resources.data?.length === 0 && (
          <Empty>暂无资源，请点击右上角「上传资源」</Empty>
        )}

        {resources.data && resources.data.length > 0 && (
          <div className="overflow-x-auto">
            <table className="table">
              <thead>
                <tr>
                  <th>显示名</th>
                  <th>指纹</th>
                  <th className="text-right">大小</th>
                  <th>上传于</th>
                  <th>所在池</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {resources.data.map((resource) => (
                  <tr key={resource.sha256} className="hover">
                    <td className="font-medium">{resource.name ?? '—'}</td>
                    <td>
                      <HashChip hash={resource.sha256} />
                    </td>
                    <td className="text-right tabular-nums text-sm">
                      {formatBytes(resource.size)}
                    </td>
                    <td className="text-xs opacity-60">{formatTime(resource.created_at)}</td>
                    <td>
                      {resource.pools.length === 0 ? (
                        <span className="badge badge-warning badge-soft badge-xs">仅本地</span>
                      ) : (
                        <div className="flex flex-wrap gap-1">
                          {resource.pools.map((pool) => (
                            <span key={pool} className="badge badge-ghost badge-xs font-mono">
                              {pool}
                            </span>
                          ))}
                        </div>
                      )}
                    </td>
                    <td className="text-right">
                      <div className="join">
                        <button
                          className="btn btn-xs join-item"
                          onClick={() => setDetailSha(resource.sha256)}
                        >
                          详情
                        </button>
                        <button
                          className="btn btn-xs btn-error join-item"
                          onClick={() => {
                            void askConfirm(
                              '确认删除该资源？本地副本与资源池副本将一并删除。',
                            ).then((ok) => {
                              if (ok) void run(() => client.deleteResource(resource.sha256));
                            });
                          }}
                        >
                          删除
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <UploadModal
        client={client}
        open={uploadOpen}
        onClose={() => setUploadOpen(false)}
        onDone={(sha256) => {
          setUploadOpen(false);
          resources.reload();
          setDetailSha(sha256);
        }}
      />

      <ResourceDetailModal
        client={client}
        sha256={detailSha}
        onClose={() => setDetailSha(null)}
        onRenamed={() => resources.reload()}
      />

      {confirmEl}
    </section>
  );
}

/** 上传分步：选择文件 → 设定资源名 → 确定开始。完成后自动弹详情。 */
function UploadModal({
  client,
  open,
  onClose,
  onDone,
}: {
  client: ApiClient;
  open: boolean;
  onClose: () => void;
  onDone: (sha256: string) => void;
}) {
  const [files, setFiles] = useState<SelectedFile[]>([]);
  const [name, setName] = useState('');
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<Record<string, 'uploading' | 'ok' | 'error'>>({});
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const uploading = Object.values(progress).includes('uploading');
  const canStart = files.length > 0 && !running;

  function pick(filesList: FileList | null) {
    // 单文件上传：再次选择即替换
    const file = filesList?.[0];
    if (!file) return;
    setFiles([{ file, key: `${file.name}-${file.size}-${file.lastModified}` }]);
    if (fileRef.current) fileRef.current.value = '';
  }

  async function start() {
    if (!canStart) return;
    setRunning(true);
    setError(null);
    let lastOk: string | null = null;
    for (const { file, key } of files) {
      setProgress((p) => ({ ...p, [key]: 'uploading' }));
      try {
        const result = await client.uploadResource(file, name.trim() || undefined);
        setProgress((p) => ({ ...p, [key]: 'ok' }));
        lastOk = result.sha256;
      } catch (e) {
        setProgress((p) => ({ ...p, [key]: 'error' }));
        setError(e instanceof Error ? e.message : String(e));
      }
    }
    setRunning(false);
    if (lastOk) onDone(lastOk);
  }

  return (
    <Modal
      open={open}
      title="上传资源"
      onClose={() => {
        if (!uploading) onClose();
      }}
      showCancel={false}
      footer={
        <button className="btn" onClick={onClose} disabled={uploading}>
          取消
        </button>
      }
    >
      <div className="space-y-3">
        <Field label="资源名（可选，可后续修改）">
          <input
            className="input w-full font-mono text-xs"
            placeholder="仅管理端识别用"
            value={name}
            disabled={running}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>

        <input
          ref={fileRef}
          type="file"
          disabled={running}
          className="file-input file-input-sm w-full"
          onChange={(e) => pick(e.target.files)}
        />

        {files.length > 0 && (
          <div className="space-y-1 border border-base-300 bg-base-200 p-2">
            {files.map(({ file, key }) => {
              const state = progress[key];
              return (
                <div key={key} className="flex items-center gap-2 px-1 py-0.5 text-sm">
                  <span className="grow truncate">{file.name}</span>
                  <span className="tabular-nums text-xs opacity-60">{formatBytes(file.size)}</span>
                  {state === 'uploading' && <span className="loading loading-spinner loading-xs" />}
                  {state === 'ok' && <span className="text-success">✓</span>}
                  {state === 'error' && <span className="text-error">✗</span>}
                  {!running && !state && (
                    <button
                      className="btn btn-ghost btn-xs text-error"
                      onClick={() => setFiles((prev) => prev.filter((item) => item.key !== key))}
                    >
                      移除
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        )}

        <button
          className="btn btn-primary w-full"
          disabled={!canStart}
          onClick={() => void start()}
        >
          {running ? '上传中…' : '开始上传'}
        </button>
        {error && <p className="text-error text-xs">{error}</p>}
      </div>
    </Modal>
  );
}

function ResourceDetailModal({
  client,
  sha256,
  onClose,
  onRenamed,
}: {
  client: ApiClient;
  sha256: string | null;
  onClose: () => void;
  onRenamed: () => void;
}) {
  const detail = useAsync(
    () =>
      sha256 === null
        ? Promise.resolve<ResourceDetailView | null>(null)
        : client.getResourceDetail(sha256),
    [client, sha256],
  );
  const [renaming, setRenaming] = useState(false);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);

  function beginRename(current: string | null) {
    setDraft(current ?? '');
    setRenaming(true);
    setError(null);
  }

  async function saveRename() {
    if (!sha256) return;
    setError(null);
    try {
      await client.renameResource(sha256, draft.trim() || undefined);
      setRenaming(false);
      detail.reload();
      onRenamed();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <Modal open={sha256 !== null} title="资源详情" onClose={onClose}>
      {detail.loading && <Loading />}
      {detail.error && <ErrorAlert message={detail.error} />}
      {detail.data && (
        <div className="space-y-3">
          <div className="flex items-baseline gap-2">
            {renaming ? (
              <input
                className="input input-sm w-64 font-mono text-xs"
                value={draft}
                autoFocus
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && void saveRename()}
              />
            ) : (
              <span className="text-lg font-bold">{detail.data.name ?? '（未命名）'}</span>
            )}
            {renaming ? (
              <button className="btn btn-primary btn-xs" onClick={() => void saveRename()}>
                保存
              </button>
            ) : (
              <button
                className="btn btn-ghost btn-xs"
                title="修改资源名"
                onClick={() => beginRename(detail.data!.name)}
              >
                重命名
              </button>
            )}
            <span className="text-base-content/60 text-sm tabular-nums">
              {formatBytes(detail.data.size)}
            </span>
          </div>
          {renaming && error && <p className="text-error text-xs">{error}</p>}

          <div>
            <span className="eyebrow block py-1">sha256 · 完整指纹可复制</span>
            <p className="break-all font-mono text-xs opacity-70">{detail.data.sha256}</p>
          </div>
          <div className="grid grid-cols-2 gap-2 text-sm">
            <div>
              <span className="block text-xs opacity-60">上传时间</span>
              <span className="font-mono">{formatTime(detail.data.created_at)}</span>
            </div>
            <div>
              <span className="block text-xs opacity-60">所在资源池</span>
              {detail.data.pools.length === 0 ? (
                <span className="badge badge-warning badge-soft badge-xs">仅本地</span>
              ) : (
                <span className="font-mono text-xs">{detail.data.pools.join(' / ')}</span>
              )}
            </div>
          </div>
          <div>
            <span className="block text-xs opacity-60">引用应用</span>
            {detail.data.ref_apps.length === 0 ? (
              <span className="text-sm opacity-60">无</span>
            ) : (
              <span className="text-sm">{detail.data.ref_apps.join('、')}</span>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}