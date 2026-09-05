import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { AnnounceView, AppView } from '../api/types';
import HashChip, { formatBytes } from '../components/HashChip';
import { PickResourceModal } from '../components/LinkResourceModal';
import { useConfirm } from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import { ErrorAlert, Loading } from '../components/States';
import { useAsync } from '../hooks/useAsync';
import ChannelsPage from './ChannelsPage';
import { period, statusBadge } from './AnnouncesPage';

type View = 'overview' | 'channels' | 'resources' | 'announces';

/** App 详情：左侧栏导航，四个视图都按当前 app 关联/过滤。 */
export default function AppDetailPage({
  client,
  app,
}: {
  client: ApiClient;
  app: AppView;
}) {
  const [view, setView] = useState<View>('overview');
  const [linkingResource, setLinkingResource] = useState(false);
  const [linkingAnnounce, setLinkingAnnounce] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmEl, askConfirm] = useConfirm();

  const channels = useAsync(() => client.listChannelsAdmin(app.id), [client, app.id]);
  const linked = useAsync(() => client.listAppResources(app.id), [client, app.id]);
  const announces = useAsync(() => client.listAppAnnounces(app.id), [client, app.id]);

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      linked.reload();
      announces.reload();
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    }
  }

  const NAV: [View, string, string][] = [
    ['overview', '概览', ''],
    ['channels', '发版通道', `${channels.data?.length ?? 0}`],
    ['resources', '资源', `${linked.data?.length ?? 0}`],
    ['announces', '公告', `${announces.data?.length ?? 0}`],
  ];

  return (
    <div className="grid gap-4 lg:grid-cols-[13rem_1fr]">
      <aside className="card h-fit border border-base-300 bg-base-100">
        <div className="card-body gap-3 p-4">
          <div className="border-b border-base-300 pb-3">
            <div className="flex items-center gap-2">
              <span className="text-lg font-bold">{app.name}</span>
              <span
                className={`badge badge-soft ${app.enabled ? 'badge-success' : 'badge-ghost'}`}
              >
                {app.enabled ? '启用' : '禁用'}
              </span>
            </div>
            <HashChip hash={app.id} head={10} />
          </div>
          <nav className="flex flex-col gap-1">
            {NAV.map(([value, label, count]) => (
              <button
                key={value}
                className={`btn btn-sm justify-between ${view === value ? 'btn-active' : 'btn-ghost'}`}
                onClick={() => setView(value)}
              >
                <span>{label}</span>
                {count !== '' && <span className="text-xs opacity-60">{count}</span>}
              </button>
            ))}
          </nav>
        </div>
      </aside>

      <main className="min-w-0">
        {error && <ErrorAlert message={error} />}

        {view === 'overview' && (
          <section className="card border border-base-300 bg-base-100">
            <div className="card-body gap-4">
              <h2 className="card-title">概览</h2>
              <div className="stats stats-vertical border border-base-300 bg-base-100 sm:stats-horizontal">
                <div className="stat">
                  <div className="stat-title">发版通道</div>
                  <div className="stat-value font-mono">{channels.data?.length ?? '—'}</div>
                  <div className="stat-desc">
                    {channels.data && channels.data.length > 0
                      ? `默认：${channels.data.find((c) => c.is_default)?.tag_name ?? '无'}`
                      : '尚未创建'}
                  </div>
                </div>
                <div className="stat">
                  <div className="stat-title">关联资源</div>
                  <div className="stat-value font-mono">{linked.data?.length ?? '—'}</div>
                  <div className="stat-desc">
                    {linked.data && linked.data.length > 0
                      ? `共 ${formatBytes(linked.data.reduce((sum, r) => sum + r.size, 0))}`
                      : '尚未关联'}
                  </div>
                </div>
                <div className="stat">
                  <div className="stat-title">公告</div>
                  <div className="stat-value font-mono">{announces.data?.length ?? '—'}</div>
                  <div className="stat-desc">
                    {announces.data && announces.data.length > 0
                      ? `最新：${announces.data[0].title}`
                      : '尚未引用'}
                  </div>
                </div>
              </div>
              <p className="text-base-content/60 text-sm">
                可通过左侧栏查看本应用的发版通道、资源与公告；资源与公告均为全局内容，关联仅为引用。
              </p>
            </div>
          </section>
        )}

        {view === 'channels' && <ChannelsPage client={client} appId={app.id} />}

        {view === 'resources' && (
          <section className="card border border-base-300 bg-base-100">
            <div className="card-body gap-4">
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="card-title">关联资源</h2>
                  <p className="text-base-content/60 text-sm">
                    引用全局资源库内容；解绑不影响资源本身
                  </p>
                </div>
                <button className="btn btn-sm btn-primary" onClick={() => setLinkingResource(true)}>
                  关联资源
                </button>
              </div>
              {linked.loading && <Loading />}
              {!linked.loading && linked.data?.length === 0 && (
                <p className="text-base-content/60 py-6 text-center text-sm">
                  尚未关联任何资源；请先在「资源」页上传，再执行关联
                </p>
              )}
              {linked.data && linked.data.length > 0 && (
                <div className="overflow-x-auto">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>指纹</th>
                        <th className="text-right">大小</th>
                        <th className="text-right">操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {linked.data.map((resource) => (
                        <tr key={resource.sha256} className="hover">
                          <td>
                            <HashChip hash={resource.sha256} />
                          </td>
                          <td className="text-right tabular-nums text-sm">
                            {formatBytes(resource.size)}
                          </td>
                          <td className="text-right">
                            <button
                              className="btn btn-xs btn-error"
                              onClick={() => {
                                void askConfirm(
                                  '确认解除关联？资源本身不会被删除。',
                                ).then((ok) => {
                                  if (ok)
                                    void run(() =>
                                      client.unlinkAppResource(app.id, resource.sha256),
                                    );
                                });
                              }}
                            >
                              解绑
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </section>
        )}

        {view === 'announces' && (
          <section className="card border border-base-300 bg-base-100">
            <div className="card-body gap-4">
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="card-title">公告</h2>
                  <p className="text-base-content/60 text-sm">
                    引用全局公告库内容；多条应用可共享同一公告
                  </p>
                </div>
                <button className="btn btn-sm btn-primary" onClick={() => setLinkingAnnounce(true)}>
                  关联公告
                </button>
              </div>
              {announces.loading && <Loading />}
              {!announces.loading && announces.data?.length === 0 && (
                <p className="text-base-content/60 py-6 text-center text-sm">
                  尚未引用任何公告；请先在「公告」页发布，再执行关联
                </p>
              )}
              {announces.data && announces.data.length > 0 && (
                <div className="overflow-x-auto">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>标题</th>
                        <th className="max-w-60">内容</th>
                        <th>有效期</th>
                        <th>状态</th>
                        <th className="text-right">操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {announces.data.map((announce) => {
                        const badge = statusBadge(announce.status);
                        return (
                          <tr key={announce.guid} className="hover">
                            <td className="font-medium">{announce.title}</td>
                            <td className="max-w-60 truncate text-sm opacity-70">
                              {announce.content}
                            </td>
                            <td className="text-xs opacity-70">
                              {period(announce.starts_at, announce.expires_at)}
                            </td>
                            <td>
                              <span className={`badge badge-soft ${badge.className}`}>
                                {badge.label}
                              </span>
                            </td>
                            <td className="text-right">
                              <button
                                className="btn btn-xs btn-error"
                                onClick={() => {
                                  void askConfirm(
                                    '确认解除关联？公告本身不会被删除。',
                                  ).then((ok) => {
                                    if (ok)
                                      void run(() =>
                                        client.unlinkAppAnnounce(app.id, announce.guid),
                                      );
                                  });
                                }}
                              >
                                解绑
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </section>
        )}
      </main>

      <PickResourceModal
        client={client}
        appId={app.id}
        open={linkingResource}
        onClose={() => setLinkingResource(false)}
        onLink={async (body) => {
          if (await run(() => client.linkAppResource(body.app_id, body.sha256))) {
            setLinkingResource(false);
          }
        }}
      />

      <LinkAnnounceModal
        linkedGuids={new Set((announces.data ?? []).map((announce) => announce.guid))}
        client={client}
        open={linkingAnnounce}
        onClose={() => setLinkingAnnounce(false)}
        onLink={async (guid) => {
          if (await run(() => client.linkAppAnnounce(app.id, guid))) {
            setLinkingAnnounce(false);
          }
        }}
      />

      {confirmEl}
    </div>
  );
}

/** 从全局公告库挑一条关联（已关联的置灰不可选） */
function LinkAnnounceModal({
  client,
  linkedGuids,
  open,
  onClose,
  onLink,
}: {
  client: ApiClient;
  linkedGuids: Set<string>;
  open: boolean;
  onClose: () => void;
  onLink: (guid: string) => Promise<void>;
}) {
  const all = useAsync(() => (open ? client.listAnnounces() : Promise.resolve<AnnounceView[]>([])), [client, open]);
  const [selected, setSelected] = useState('');
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const candidates = (all.data ?? []).filter((announce) => !linkedGuids.has(announce.guid));

  return (
    <Modal
      open
      title="关联公告"
      onClose={onClose}
      footer={
        <button
          className="btn btn-primary"
          disabled={selected === ''}
          onClick={() => {
            setError(null);
            void onLink(selected);
          }}
        >
          关联
        </button>
      }
    >
      {all.loading && <Loading />}
      {all.error && <ErrorAlert message={all.error} />}
      {!all.loading && candidates.length === 0 && (
        <p className="text-base-content/60 text-sm">
          暂无可用公告（已全部关联或公告库为空）
        </p>
      )}
      {!all.loading && candidates.length > 0 && (
        <select
          className="select w-full"
          value={selected}
          onChange={(e) => setSelected(e.target.value)}
        >
          <option value="">选择公告</option>
          {candidates.map((announce) => {
            const badge = statusBadge(announce.status);
            return (
              <option key={announce.guid} value={announce.guid}>
                {announce.title} · {announce.guid.slice(0, 8)} · {badge.label}
              </option>
            );
          })}
        </select>
      )}
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}