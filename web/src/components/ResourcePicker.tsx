import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { ResourceView } from '../api/types';
import HashChip, { formatBytes } from './HashChip';
import Modal from './Modal';
import { Empty, ErrorAlert, Field, Loading } from './States';
import { useAsync } from '../hooks/useAsync';

/**
 * 通用资源选择：搜索全局资源库（sha256 前缀过滤）选中即回填。
 * 替代任何手动输入 sha256 的表单场景（通道完整包 / 差分 / 关联等）。
 */
export default function ResourcePicker({
  client,
  value,
  onChange,
  disabled,
  allowClear = false,
}: {
  client: ApiClient;
  /** 当前选中的 sha256，null 未选 */
  value: string | null;
  onChange: (sha256: string | null) => void;
  disabled?: boolean;
  /** 允许清除当前选择 */
  allowClear?: boolean;
}) {
  const [open, setOpen] = useState(false);

  return (
    <div className="flex items-center gap-2">
      <div className="flex min-w-0 grow items-center gap-2">
        {value ? (
          <HashChip hash={value} head={16} />
        ) : (
          <span className="text-sm opacity-60">未选择资源</span>
        )}
        {value && allowClear && !disabled && (
          <button
            type="button"
            className="btn btn-ghost btn-xs text-error"
            onClick={() => onChange(null)}
          >
            清除
          </button>
        )}
      </div>
      <button
        type="button"
        className="btn btn-sm"
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        选择资源
      </button>

      <ResourcePickerModal
        client={client}
        open={open}
        onClose={() => setOpen(false)}
        onPick={(sha256) => {
          onChange(sha256);
          setOpen(false);
        }}
      />
    </div>
  );
}

/** 搜索 + 点选资源，回传 sha256。供 ResourcePicker 内部使用。 */
export function ResourcePickerModal({
  client,
  open,
  onClose,
  onPick,
}: {
  client: ApiClient;
  open: boolean;
  onClose: () => void;
  onPick: (sha256: string) => void;
}) {
  const resources = useAsync(
    () => (open ? client.listResources() : Promise.resolve<ResourceView[]>([])),
    [client, open],
  );
  const [query, setQuery] = useState('');
  const [picked, setPicked] = useState<string>('');

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
          onClick={() => onPick(picked)}
        >
          确定
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
            <p className="px-2 py-3 text-center text-sm opacity-60">未找到匹配的资源</p>
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
                {resource.name ?? '—'} · {formatBytes(resource.size)} · {resource.pools.join('/')}
              </span>
            </label>
          ))}
        </div>
      )}
    </Modal>
  );
}