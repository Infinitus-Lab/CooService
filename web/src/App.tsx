import { useMemo, useState } from 'react';
import { createClient } from './api/client';
import type { AppView } from './api/types';
import ApiSetupDialog from './components/ApiSetupDialog';
import AnnouncesPage from './pages/AnnouncesPage';
import AppDetailPage from './pages/AppDetailPage';
import AppsPage from './pages/AppsPage';
import PoolsPage from './pages/PoolsPage';
import ResourcesPage from './pages/ResourcesPage';

type Tab = 'apps' | 'pools' | 'resources' | 'announces';

const KEY_STORAGE = 'coo.admin_key';
const BASE_STORAGE = 'coo.api_base';

export default function App() {
  const [adminKey, setAdminKey] = useState(() => sessionStorage.getItem(KEY_STORAGE) ?? '');
  const [baseApi, setBaseApi] = useState(() => sessionStorage.getItem(BASE_STORAGE) ?? '');
  const [gateOpen, setGateOpen] = useState(false);
  const [gateFocus, setGateFocus] = useState<'base' | 'key'>('base');
  const [tab, setTab] = useState<Tab>('apps');
  const [appOpen, setAppOpen] = useState<AppView | null>(null);

  // 首页不是登录页：直接渲染业务页，地址不可达或管理接口 401 才弹 API 设置框
  const client = useMemo(
    () =>
      createClient(
        baseApi,
        () => adminKey,
        () => {
          setGateFocus('key');
          setGateOpen(true);
        },
        () => {
          setGateFocus('base');
          setGateOpen(true);
        },
      ),
    [baseApi, adminKey],
  );

  async function verify(base: string, key: string): Promise<void> {
    const probe = createClient(base, () => key, () => {}, () => {});
    await probe.listApps();
    sessionStorage.setItem(BASE_STORAGE, base);
    sessionStorage.setItem(KEY_STORAGE, key);
    setBaseApi(base);
    setAdminKey(key);
    setGateOpen(false);
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
          <span className="font-mono text-xs opacity-50">{baseApi || '同源'}</span>
          <span
            className={`badge badge-soft font-mono ${adminKey ? 'badge-success' : 'badge-warning'}`}
            title={adminKey ? '管理密钥已配置' : '未配置：触发管理请求时将要求输入密钥'}
          >
            {adminKey ? `密钥 ${adminKey.slice(0, 8)}…` : '未配置密钥'}
          </span>
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => {
              setGateFocus(baseApi ? 'key' : 'base');
              setGateOpen(true);
            }}
          >
            设置
          </button>
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
        CooService 管理台 · API 地址与管理密钥仅保存在当前标签页（sessionStorage），地址不可达或接口返回
        401 时将要求重新配置
      </footer>

      {gateOpen && (
        <ApiSetupDialog
          initialBase={baseApi}
          initialKey={adminKey}
          focus={gateFocus}
          onSubmit={verify}
        />
      )}
    </div>
  );
}