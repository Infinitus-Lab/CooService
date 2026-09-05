import { useMemo, useState } from 'react';
import { ApiError, BASE_API, createClient } from './api/client';
import type { AppView } from './api/types';
import AnnouncesPage from './pages/AnnouncesPage';
import AppDetailPage from './pages/AppDetailPage';
import AppsPage from './pages/AppsPage';
import PoolsPage from './pages/PoolsPage';
import ResourcesPage from './pages/ResourcesPage';

type Tab = 'apps' | 'pools' | 'resources' | 'announces';

const KEY_STORAGE = 'coo.admin_key';

export default function App() {
  const [adminKey, setAdminKey] = useState(() => sessionStorage.getItem(KEY_STORAGE) ?? '');
  const [gateOpen, setGateOpen] = useState(false);
  const [tab, setTab] = useState<Tab>('apps');
  const [appOpen, setAppOpen] = useState<AppView | null>(null);

  // 首页不是登录页：直接渲染业务页，任何管理接口 401 才弹密钥门
  const client = useMemo(
    () => createClient(() => adminKey, () => setGateOpen(true)),
    [adminKey],
  );

  async function verify(key: string): Promise<unknown> {
    const probe = createClient(() => key, () => {});
    const result = await probe.listApps();
    sessionStorage.setItem(KEY_STORAGE, key);
    setAdminKey(key);
    setGateOpen(false);
    return result;
  }

  function disconnect() {
    sessionStorage.removeItem(KEY_STORAGE);
    setAdminKey('');
    setAppOpen(null);
  }

  return (
    <div className="flex min-h-screen flex-col">
      <header className="navbar border-b border-base-300 bg-base-100 px-4">
        <div className="navbar-start min-w-0">
          <span className="font-mono text-sm font-bold tracking-[0.22em]">
            COO<span className="text-primary">SERVICE</span>
          </span>
          <span className="text-primary text-lg leading-none">●</span>

          {appOpen ? (
            <div className="ml-4 min-w-0">
              <div className="breadcrumbs text-sm">
                <ul>
                  <li>
                    <button onClick={() => setAppOpen(null)}>应用</button>
                  </li>
                  <li className="min-w-0 font-medium text-base-content">
                    <span className="block max-w-48 truncate" title={appOpen.name}>
                      {appOpen.name}
                    </span>
                  </li>
                </ul>
              </div>
            </div>
          ) : (
            <nav className="ml-4">
              <ul className="menu menu-horizontal gap-1 px-1">
                <li className={tab === 'apps' ? 'menu-active' : ''}>
                  <button onClick={() => setTab('apps')}>应用</button>
                </li>
                <li className={tab === 'pools' || tab === 'resources' ? 'menu-active' : ''}>
                  <details>
                    <summary>资源管理</summary>
                    <ul className="menu dropdown-content z-20 w-40 rounded-box border border-base-300 bg-base-100 p-1 shadow">
                      <li className={tab === 'pools' ? 'menu-active' : ''}>
                        <button onClick={() => setTab('pools')}>资源池</button>
                      </li>
                      <li className={tab === 'resources' ? 'menu-active' : ''}>
                        <button onClick={() => setTab('resources')}>资源</button>
                      </li>
                    </ul>
                  </details>
                </li>
                <li className={tab === 'announces' ? 'menu-active' : ''}>
                  <button onClick={() => setTab('announces')}>公告</button>
                </li>
              </ul>
            </nav>
          )}
        </div>

        <div className="navbar-end">
          <span className="font-mono text-xs opacity-50">{BASE_API || '同源'}</span>
          <span
            className={`badge badge-soft font-mono ${adminKey ? 'badge-success' : 'badge-warning'}`}
            title={adminKey ? '管理密钥已配置' : '未配置：触发管理请求时将要求输入密钥'}
          >
            {adminKey ? `密钥 ${adminKey.slice(0, 8)}…` : '未配置密钥'}
          </span>
          <button className="btn btn-ghost btn-sm" onClick={disconnect}>
            断开
          </button>
        </div>
      </header>

      <div className="mx-auto w-full max-w-6xl flex-1 px-4 py-6">
        {appOpen ? (
          <AppDetailPage client={client} app={appOpen} />
        ) : (
          <>
            {tab === 'apps' && <AppsPage client={client} onOpen={setAppOpen} />}
            {tab === 'pools' && <PoolsPage client={client} />}
            {tab === 'resources' && <ResourcesPage client={client} />}
            {tab === 'announces' && <AnnouncesPage client={client} />}
          </>
        )}
      </div>

      <footer className="footer footer-center text-base-content/50 p-4 text-xs">
        CooService 管理台 · 管理密钥仅保存在当前标签页，接口返回 401 时将重新要求密钥
      </footer>

      {gateOpen && <KeyGate onSubmit={verify} />}
    </div>
  );
}

function KeyGate({ onSubmit }: { onSubmit: (key: string) => Promise<unknown> }) {
  const [draft, setDraft] = useState('');
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    setChecking(true);
    setError(null);
    try {
      await onSubmit(draft.trim());
    } catch (e) {
      setError(
        e instanceof ApiError && e.code === 401
          ? '密钥无效，请从服务启动日志重新获取'
          : e instanceof ApiError
            ? e.message
            : `连不上服务（${BASE_API || '同源'}）`,
      );
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-base-200 p-4">
      <div className="w-full max-w-md border border-base-300 bg-base-100 p-6">
        <div className="mb-1 flex items-baseline justify-between">
          <span className="font-mono text-sm font-bold tracking-[0.22em]">
            COOSERVICE · KEYGATE
          </span>
          <span className="text-base-content/50 text-xs">X-Admin-Key</span>
        </div>
        <p className="mb-4 text-sm opacity-60">
          管理密钥在服务每次启动时随机生成，仅打印于启动日志；粘贴后本页接口将自动恢复
        </p>
        <input
          type="password"
          autoFocus
          className="input w-full font-mono text-sm"
          placeholder="粘贴启动日志里的 admin_key"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !checking && draft.trim() !== '') void submit();
          }}
        />
        {error && <p className="text-error mt-2 text-xs">{error}</p>}
        <button
          className="btn btn-primary mt-4 w-full"
          disabled={draft.trim() === '' || checking}
          onClick={() => void submit()}
        >
          {checking ? <span className="loading loading-spinner loading-sm" /> : null}
          {checking ? '验证中…' : '进入'}
        </button>
      </div>
    </div>
  );
}