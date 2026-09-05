import { useState } from 'react';
import type { ApiClient } from '../api/client';
import type { AnnounceView } from '../api/types';
import HashChip, { formatTime } from '../components/HashChip';
import { useConfirm } from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import { Empty, ErrorAlert, Field, Loading } from '../components/States';
import { useAsync } from '../hooks/useAsync';

/** datetime-local 输入值 → ISO（空串 = null = 不限） */
export function toIso(local: string): string | null {
  return local ? new Date(local).toISOString() : null;
}

/** ISO → datetime-local 输入值（只到分钟精度） */
export function fromIso(iso: string | null | undefined): string {
  return iso ? iso.slice(0, 16) : '';
}

export function statusBadge(status: string) {
  switch (status) {
    case 'active':
      return { className: 'badge-success', label: '展示中' };
    case 'scheduled':
      return { className: 'badge-warning', label: '未开始' };
    case 'expired':
      return { className: 'badge-ghost', label: '已过期' };
    default:
      return { className: 'badge-neutral', label: '长期有效' };
  }
}

export function period(startsAt: string | null, expiresAt: string | null): string {
  if (!startsAt && !expiresAt) return '—';
  const start = startsAt ? formatTime(startsAt) : '不限';
  const end = expiresAt ? formatTime(expiresAt) : '不限';
  return `${start} → ${end}`;
}

/** 公告台账：全局内容，app 通过引用关联（多 app 可共享同一条）。 */
export default function AnnouncesPage({ client }: { client: ApiClient }) {
  const apps = useAsync(() => client.listApps(), [client]);
  const [filter, setFilter] = useState('');
  const list = useAsync(
    () => client.listAnnounces(filter || undefined),
    [client, filter],
  );
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<AnnounceView | null>(null);
  const [confirmEl, askConfirm] = useConfirm();
  const [error, setError] = useState<string | null>(null);

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      list.reload();
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
            <h2 className="card-title">公告</h2>
            <p className="text-base-content/60 text-sm">
              全局内容，由应用引用；仅在有效期内对客户端展示
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <select
              className="select select-sm w-44"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            >
              <option value="">全部公告</option>
              {apps.data?.map((app) => (
                <option key={app.id} value={app.id}>
                  {app.name}
                </option>
              ))}
            </select>
            <button className="btn btn-sm" onClick={list.reload} disabled={list.loading}>
              刷新
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setCreating(true)}>
              发布公告
            </button>
          </div>
        </div>

        {error && <ErrorAlert message={error} />}

        {list.loading && <Loading />}
        {!list.loading && list.data?.length === 0 && <Empty>暂无公告，请点击右上角「发布公告」</Empty>}

        {list.data && list.data.length > 0 && (
          <div className="overflow-x-auto">
            <table className="table">
              <thead>
                <tr>
                  <th>标题</th>
                  <th className="max-w-60">内容</th>
                  <th>有效期</th>
                  <th>状态</th>
                  <th>引用应用</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {list.data.map((announce) => {
                  const badge = statusBadge(announce.status);
                  return (
                    <tr key={announce.guid} className="hover">
                      <td className="font-medium">{announce.title}</td>
                      <td className="max-w-60 truncate text-sm opacity-70">{announce.content}</td>
                      <td className="text-xs opacity-70">
                        {period(announce.starts_at, announce.expires_at)}
                      </td>
                      <td>
                        <span className={`badge badge-soft ${badge.className}`}>{badge.label}</span>
                      </td>
                      <td className="text-sm">
                        {announce.ref_apps.length > 0 ? announce.ref_apps.join('、') : '—'}
                      </td>
                      <td className="text-right">
                        <div className="join">
                          <button
                            className="btn btn-xs join-item"
                            onClick={() => setEditing(announce)}
                          >
                            编辑
                          </button>
                          <button
                            className="btn btn-xs btn-error join-item"
                            onClick={() => {
                              void askConfirm(`确认删除公告「${announce.title}」？`).then((ok) => {
                                if (ok) void run(() => client.deleteAnnounce(announce.guid));
                              });
                            }}
                          >
                            删除
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <CreateAnnounceModal
        open={creating}
        onClose={() => setCreating(false)}
        onCreate={async (body) => {
          if (await run(() => client.createAnnounce(body))) setCreating(false);
        }}
      />

      <EditAnnounceModal
        announce={editing}
        onClose={() => setEditing(null)}
        onSave={async (guid, body) => {
          if (await run(() => client.updateAnnounce(guid, body))) setEditing(null);
        }}
      />

      {confirmEl}
    </section>
  );
}

interface AnnounceForm {
  title: string;
  content: string;
  starts_at: string;
  expires_at: string;
}

export function validatePeriod(startsAt: string, expiresAt: string): string | null {
  if (startsAt && expiresAt && new Date(startsAt) >= new Date(expiresAt)) {
    return '展示终点必须晚于起点';
  }
  return null;
}

function CreateAnnounceModal({
  open,
  onClose,
  onCreate,
}: {
  open: boolean;
  onClose: () => void;
  onCreate: (body: {
    title: string;
    content: string;
    starts_at: string | null;
    expires_at: string | null;
  }) => Promise<void>;
}) {
  const [form, setForm] = useState<AnnounceForm>({ title: '', content: '', starts_at: '', expires_at: '' });
  const [error, setError] = useState<string | null>(null);

  function submit() {
    if (!form.title.trim() || !form.content.trim()) {
      setError('标题、内容必填');
      return;
    }
    const problem = validatePeriod(form.starts_at, form.expires_at);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    void onCreate({
      title: form.title.trim(),
      content: form.content.trim(),
      starts_at: toIso(form.starts_at),
      expires_at: toIso(form.expires_at),
    }).then(() => setForm((f) => ({ ...f, title: '', content: '', starts_at: '', expires_at: '' })));
  }

  return (
    <Modal
      open={open}
      title="发布公告"
      onClose={onClose}
      footer={
        <button className="btn btn-primary" onClick={submit}>
          发布
        </button>
      }
    >
      <Field label="标题" required>
        <input
          className="input w-full"
          value={form.title}
          onChange={(e) => setForm({ ...form, title: e.target.value })}
        />
      </Field>
      <Field label="内容" required>
        <textarea
          className="textarea w-full"
          rows={4}
          value={form.content}
          onChange={(e) => setForm({ ...form, content: e.target.value })}
        />
      </Field>
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="展示起始时间（留空表示立即生效）">
          <input
            type="datetime-local"
            className="input w-full font-mono text-xs"
            value={form.starts_at}
            onChange={(e) => setForm({ ...form, starts_at: e.target.value })}
          />
        </Field>
        <Field label="展示截止时间（留空表示不限）">
          <input
            type="datetime-local"
            className="input w-full font-mono text-xs"
            value={form.expires_at}
            onChange={(e) => setForm({ ...form, expires_at: e.target.value })}
          />
        </Field>
      </div>
      <p className="text-xs opacity-60">发布后需在应用详情页关联，客户端方可查看</p>
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}

function EditAnnounceModal({
  announce,
  onClose,
  onSave,
}: {
  announce: AnnounceView | null;
  onClose: () => void;
  onSave: (guid: string, body: {
    title: string;
    content: string;
    starts_at: string | null;
    expires_at: string | null;
  }) => Promise<void>;
}) {
  const [title, setTitle] = useState(announce?.title ?? '');
  const [content, setContent] = useState(announce?.content ?? '');
  const [startsAt, setStartsAt] = useState(fromIso(announce?.starts_at));
  const [expiresAt, setExpiresAt] = useState(fromIso(announce?.expires_at));
  const [error, setError] = useState<string | null>(null);

  if (!announce) return null;
  const target = announce;

  function submit() {
    if (!title.trim() || !content.trim()) {
      setError('标题、内容必填');
      return;
    }
    const problem = validatePeriod(startsAt, expiresAt);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    // PATCH 全量语义：四个字段一次给全
    void onSave(target.guid, {
      title: title.trim(),
      content: content.trim(),
      starts_at: toIso(startsAt),
      expires_at: toIso(expiresAt),
    });
  }

  return (
    <Modal
      open
      title={`编辑公告：${target.title}`}
      onClose={onClose}
      footer={
        <button className="btn" onClick={submit}>
          保存
        </button>
      }
    >
      <p className="break-all text-xs opacity-60">
        公告 Id：<HashChip hash={target.guid} head={10} />
      </p>
      <Field label="标题" required>
        <input
          className="input w-full"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
      </Field>
      <Field label="内容" required>
        <textarea
          className="textarea w-full"
          rows={4}
          value={content}
          onChange={(e) => setContent(e.target.value)}
        />
      </Field>
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="展示起始时间（清空表示立即生效）">
          <input
            type="datetime-local"
            className="input w-full font-mono text-xs"
            value={startsAt}
            onChange={(e) => setStartsAt(e.target.value)}
          />
        </Field>
        <Field label="展示截止时间（清空表示不限）">
          <input
            type="datetime-local"
            className="input w-full font-mono text-xs"
            value={expiresAt}
            onChange={(e) => setExpiresAt(e.target.value)}
          />
        </Field>
      </div>
      <p className="text-xs opacity-60">
        已引用应用：{target.ref_apps.length > 0 ? target.ref_apps.join('、') : '无'}
      </p>
      {error && <p className="text-error text-xs">{error}</p>}
    </Modal>
  );
}