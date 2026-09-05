import { useEffect, useRef, useState } from 'react';

/** 64 位十六进制 sha256 */
export const isSha256 = (value: string): boolean => /^[0-9a-f]{64}$/i.test(value);

/** 短哈希：前 12 + 后 4，展示与复制两不误 */
export function shortHash(hash: string, head = 12): string {
  if (hash.length <= head + 4) return hash;
  return `${hash.slice(0, head)}…${hash.slice(-4)}`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KiB', 'MiB', 'GiB', 'TiB'];
  let value = bytes;
  let unit = 'B';
  for (const next of units) {
    if (value < 1024) break;
    value /= 1024;
    unit = next;
  }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${unit}`;
}

export function formatTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/**
 * 哈希指纹：身份即 sha256。点击复制整串，复制成功短暂变绿。
 * 所有出现哈希的地方统一用它的展示与交互，保持内容寻址体验一致。
 */
export default function HashChip({ hash, head = 12 }: { hash: string; head?: number }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  function copy() {
    void navigator.clipboard.writeText(hash).then(() => {
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 1200);
    });
  }

  return (
    <span className="tooltip tooltip-top" data-tip={copied ? '已复制' : hash}>
      <button
        type="button"
        className={`hash-chip ${copied ? 'copied' : ''}`}
        onClick={copy}
      >
        {copied ? '已复制' : shortHash(hash, head)}
      </button>
    </span>
  );
}