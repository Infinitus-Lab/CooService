import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { AppView } from '../api/types';
import { useAsync } from '../hooks/useAsync';
import { useConfirm } from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import { Empty, ErrorAlert, Field, Loading } from '../components/States';

export default function AppsPage({
  client,
  onOpen,
}: {
  client: ApiClient;
  onOpen: (app: AppView) => void;
}) {
  const apps = useAsync(() => client.listApps(), [client]);
  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<AppView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmEl, askConfirm] = useConfirm();

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      apps.reload();
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
            <h2 className="card-title">应用</h2>
            <p className="text-base-content/60 text-sm">全部应用记录（含已禁用）</p>
          </div>
          <div className="flex gap-2">
            <button className="btn btn-sm" onClick={apps.reload} disabled={apps.loading}>
              刷新
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setCreating(true)}>
              新建应用
            </button>
          </div>
        </div>

        {error && <ErrorAlert message={error} />}

        {apps.loading && <Loading />}
        {apps.data?.length === 0 && <Empty>暂无应用，请点击右上角「新建应用」</Empty>}

        {apps.data && apps.data.length > 0 && (
          <div className="overflow-x-auto">
            <table className="table">
              <thead>
                <tr>
                  <th>名称</th>
                  <th>App Id</th>
                  <th>状态</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {apps.data.map((app) => (
                  <tr key={app.id} className="hover">
                    <td className="font-medium">{app.name}</td>
                    <td className="font-mono text-xs opacity-70">{app.id}</td>
                    <td>
                      <span
                        className={`badge badge-soft ${app.enabled ? 'badge-success' : 'badge-ghost'}`}
                      >
                        {app.enabled ? '启用' : '禁用'}
                      </span>
                    </td>
                    <td className="text-right">
                      <div className="join">
                        <button
                          className="btn btn-xs join-item"
                          onClick={() => onOpen(app)}
                        >
                          详情
                        </button>
                        <button className="btn btn-xs join-item" onClick={() => setRenaming(app)}>
                          改名
                        </button>
                        <button
                          className="btn btn-xs join-item"
                          onClick={() => run(() => client.updateApp(app.id, { enabled: !app.enabled }))}
                        >
                          {app.enabled ? '禁用' : '启用'}
                        </button>
                        <button
                          className="btn btn-xs btn-error join-item"
                          onClick={() => {
                            void askConfirm(
                              `确认删除应用「${app.name}」？删除后不可恢复。`,
                            ).then((ok) => {
                              if (ok) void run(() => client.deleteApp(app.id));
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

      <CreateAppModal
        open={creating}
        onClose={() => setCreating(false)}
        onCreate={async (name) => {
          if (await run(() => client.createApp({ name }))) setCreating(false);
        }}
      />

      <RenameAppModal
        app={renaming}
        onClose={() => setRenaming(null)}
        onRename={async (id, name) => {
          if (await run(() => client.updateApp(id, { name }))) setRenaming(null);
        }}
      />

      {confirmEl}
    </section>
  );
}

function CreateAppModal({
  open,
  onClose,
  onCreate,
}: {
  open: boolean;
  onClose: () => void;
  onCreate: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState('');

  return (
    <Modal
      open={open}
      title="新建应用"
      onClose={onClose}
      footer={
        <button
          className="btn"
          disabled={name.trim() === ''}
          onClick={() => {
            void onCreate(name.trim()).then(() => setName(''));
          }}
        >
          创建
        </button>
      }
    >
      <Field label="应用名" required>
        <input
          className="input w-full"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
      </Field>
      <p className="text-base-content/50 text-xs">App Id 可留空，由服务端生成 UUID v4</p>
    </Modal>
  );
}

function RenameAppModal({
  app,
  onClose,
  onRename,
}: {
  app: AppView | null;
  onClose: () => void;
  onRename: (id: string, name: string) => Promise<void>;
}) {
  const [name, setName] = useState(app?.name ?? '');

  if (!app) return null;

  return (
    <Modal
      open
      title={`改名：${app.name}`}
      onClose={onClose}
      footer={
        <button
          className="btn"
          disabled={name.trim() === ''}
          onClick={() => void onRename(app.id, name.trim())}
        >
          保存
        </button>
      }
    >
      <Field label="新名称">
        <input
          className="input w-full"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
      </Field>
    </Modal>
  );
}
