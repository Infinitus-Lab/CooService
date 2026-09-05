import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { AppView, ResourceView } from '../api/types';
import HashChip, { formatBytes } from './HashChip';
import Modal from './Modal';
import { Empty, ErrorAlert, Field, Loading } from './States';
import { useAsync } from '../hooks/useAsync';

/** 资源 → 选应用：从应用列表挑一个关联当前资源（纯引用，不单独命名）。 */
export default function LinkResourceModal({
  apps,
  sha256,
  open,
  onClose,
  onLink,
}: {
  apps: AppView[];
  sha256: string;
  open: boolean;
  onClose: () => void;
  onLink: (body: { app_id: string; sha256: string }) => Promise<void>;
}) {
  const [appId, setAppId] = useState(apps[0]?.id ?? '');
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  function submit() {
    if (!appId) {
      setError('请选择应用');
      return;
    }
    setError(null);
    void onLink({ app_id: appId, sha256 });
  }

  return (
    <Modal
      open
      title="关联到应用"
      onClose={onClose}
      footer={
        <button className="btn btn-primary" onClick={submit}>
          关联
        </button>
      }
    >
      <p className="break-all font-mono text-xs opacity-70">资源 sha256：{sha256}</p>
      <Field label="应用" required>
        <select
          className="select w-full"
          value={appId}
          onChange={(e) => setAppId(e.target.value)}
        >
          {apps.map((app) => (
            <option key={app.id} value={app.id}>
              {app.name}
            </option>
          ))}
        </select>
      </Field>
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}

/** 应用 → 选资源：搜索全局资源库（sha256 前缀过滤），点选后关联。 */
export function PickResourceModal({
  client,
  appId,
  open,
  onClose,
  onLink,
}: {
  client: ApiClient;
  appId: string;
  open: boolean;
  onClose: () => void;
  onLink: (body: { app_id: string; sha256: string }) => Promise<void>;
}) {
  const resources = useAsync(
    () => (open ? client.listResources() : Promise.resolve<ResourceView[]>([])),
    [client, open],
  );
  const [query, setQuery] = useState('');
  const [picked, setPicked] = useState<string>('');
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const candidates = (resources.data ?? []).filter(
    (resource) =>
      query.trim() === '' || resource.sha256.startsWith(query.trim().toLowerCase()),
  );

  return (
    <Modal
      open
      title="选择资源"
      onClose={onClose}
      footer={
        <button
          className="btn btn-primary"
          disabled={picked === ''}
          onClick={() => {
            setError(null);
            void onLink({ app_id: appId, sha256: picked });
          }}
        >
          关联
        </button>
      }
    >
      <Field label="搜索资源（sha256 前缀，不输入显示全部）">
        <input
          className="input w-full font-mono text-xs"
          placeholder="4b540700c4f1…"
          value={query}
          autoFocus
          onChange={(e) => {
            setQuery(e.target.value);
            setPicked('');
          }}
        />
      </Field>
      {resources.loading && <Loading />}
      {resources.error && <ErrorAlert message={resources.error} />}
      {!resources.loading && resources.data?.length === 0 && (
        <Empty>暂无资源，请先在「资源」页上传</Empty>
      )}
      {!resources.loading && resources.data && resources.data.length > 0 && (
        <div className="max-h-64 space-y-1 overflow-y-auto border border-base-300 bg-base-200 p-2">
          {candidates.length === 0 && (
            <p className="text-base-content/60 px-2 py-3 text-center text-sm">
              未找到匹配的资源
            </p>
          )}
          {candidates.map((resource) => (
            <label
              key={resource.sha256}
              className={`flex cursor-pointer items-center gap-2 px-2 py-1.5 hover:bg-base-300 ${
                picked === resource.sha256 ? 'bg-base-300' : ''
              }`}
            >
              <input
                type="radio"
                name="pick-resource"
                className="radio radio-primary radio-sm"
                checked={picked === resource.sha256}
                onChange={() => setPicked(resource.sha256)}
              />
              <HashChip hash={resource.sha256} head={10} />
              <span className="ml-auto text-xs tabular-nums opacity-60">
                {formatBytes(resource.size)} · {resource.pools.join('/')}
              </span>
            </label>
          ))}
        </div>
      )}
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}