import { useRef, useState } from 'react';
import { ApiError } from '../api/client';

/**
 * API 设置门：同屏输入 API 地址与管理密钥，探活成功才放行。
 * 强制 Dialog——透明遮罩，无关闭按钮/无 Esc，地址不可达或密钥 401 时业务页不可用，
 * 必须配置正确才能继续。
 */
export default function ApiSetupDialog({
  initialBase,
  initialKey,
  /** 打开时聚焦的字段：地址不可达 → 'base'，密钥 401 → 'key'，手动打开 → 'base' */
  focus,
  onSubmit,
}: {
  initialBase: string;
  initialKey: string;
  focus: 'base' | 'key';
  onSubmit: (base: string, key: string) => Promise<void>;
}) {
  const [base, setBase] = useState(initialBase);
  const [key, setKey] = useState(initialKey);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const baseRef = useRef<HTMLInputElement>(null);
  const keyRef = useRef<HTMLInputElement>(null);

  const baseValid = base.trim() === '' || /^https?:\/\//i.test(base.trim());
  const canSubmit = baseValid && key.trim() !== '' && !checking;

  async function submit() {
    setChecking(true);
    setError(null);
    try {
      await onSubmit(base.trim(), key.trim());
    } catch (e) {
      if (e instanceof ApiError && e.code === 401) {
        setError('密钥无效，请确认与部署方提供的管理密钥一致');
        keyRef.current?.focus();
      } else if (e instanceof ApiError && e.code === 0) {
        setError(e.message);
        baseRef.current?.focus();
      } else {
        setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm">
      <div className="w-full max-w-md border border-base-300 bg-base-100 p-6">
        <div className="mb-1 flex items-baseline justify-between">
          <span className="font-mono text-sm font-bold tracking-[0.22em]">
            COOSERVICE · API 设置
          </span>
          <span className="text-base-content/50 text-xs">X-Admin-Key</span>
        </div>
        <p className="mb-4 text-sm opacity-60">
          配置 API 地址与管理密钥；地址留空表示同源。仅存于当前标签页（sessionStorage）
        </p>

        <div className="space-y-3">
          <div>
            <label className="label label-text font-medium">API 地址</label>
            <input
              ref={baseRef}
              type="text"
              autoFocus={focus === 'base'}
              className="input w-full font-mono text-sm"
              placeholder="http://127.0.0.1:8081（留空 = 同源）"
              value={base}
              onChange={(e) => setBase(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && canSubmit) void submit();
              }}
            />
            {base.trim() !== '' && !baseValid && (
              <p className="text-error mt-1 text-xs">地址需以 http:// 或 https:// 开头</p>
            )}
          </div>

          <div>
            <label className="label label-text font-medium">管理密钥</label>
            <input
              ref={keyRef}
              type="password"
              autoFocus={focus === 'key'}
              className="input w-full font-mono text-sm"
              placeholder="粘贴管理密钥"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && canSubmit) void submit();
              }}
            />
          </div>
        </div>

        {error && <p className="text-error mt-2 text-xs">{error}</p>}

        <button
          className="btn btn-primary mt-4 w-full"
          disabled={!canSubmit}
          onClick={() => void submit()}
        >
          {checking ? <span className="loading loading-spinner loading-sm" /> : null}
          {checking ? '验证中…' : '保存并验证'}
        </button>
      </div>
    </div>
  );
}
