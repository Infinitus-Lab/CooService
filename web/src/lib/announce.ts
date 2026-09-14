import { formatTime } from './format';

/** datetime-local 输入值 → ISO（空串 = null = 不限） */
export function toIso(local: string): string | null {
  return local ? new Date(local).toISOString() : null;
}

/** ISO → datetime-local 输入值（只到分钟精度） */
export function fromIso(iso: string | null | undefined): string {
  return iso ? iso.slice(0, 16) : '';
}

/** 公告状态 → NuxtUI 徽章外观 */
export function statusBadge(status: string): {
  color: 'success' | 'warning' | 'neutral' | 'info';
  label: string;
} {
  switch (status) {
    case 'active':
      return { color: 'success', label: '展示中' };
    case 'scheduled':
      return { color: 'warning', label: '未开始' };
    case 'expired':
      return { color: 'neutral', label: '已过期' };
    default:
      return { color: 'info', label: '长期有效' };
  }
}

export function period(startsAt: string | null, expiresAt: string | null): string {
  if (!startsAt && !expiresAt) return '—';
  const start = startsAt ? formatTime(startsAt) : '不限';
  const end = expiresAt ? formatTime(expiresAt) : '不限';
  return `${start} → ${end}`;
}

export function validatePeriod(startsAt: string, expiresAt: string): string | null {
  if (startsAt && expiresAt && new Date(startsAt) >= new Date(expiresAt)) {
    return '展示终点必须晚于起点';
  }
  return null;
}
