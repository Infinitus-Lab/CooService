export interface AppView {
  id: string;
  name: string;
  enabled: boolean;
}

export interface PoolView {
  id: string;
  kind: string;
  endpoint: string;
  public_endpoint: string;
  config: Record<string, unknown>;
  is_public: boolean;
  /** 运行时注册表里有没有连接（建连成功） */
  connected: boolean;
  /** 健康探测是否通过 */
  up: boolean;
  health_checked_at: string | null;
  health_error: string | null;
  /** syncing / synced / error / unknown */
  sync_status: string;
  /** 待补齐的推送操作数 */
  sync_pending: number;
  sync_scanned_at: string | null;
  sync_error: string | null;
  /** 登记量统计（resource_location） */
  object_count: number;
  total_size: number;
  last_write_at: string | null;
}

// ---------- 发版通道（admin） ----------

export interface AdminChannelView {
  guid: string;
  app_id: string;
  tag_name: string;
  latest_version: string;
  raw_sha256: string;
  raw_size: number;
  is_default: boolean;
}

export interface CreateChannelRequest {
  app_id: string;
  tag_name: string;
  latest_version: string;
  latest_sha256: string;
  is_default?: boolean;
}

export interface UpdateChannelRequest {
  tag_name?: string;
  latest_version?: string;
  latest_sha256?: string;
  is_default?: boolean;
}

export interface ReleaseView {
  version: string;
  sha256: string;
  created_at: string;
}

export interface AdminDiffView {
  base_sha256: string;
  patch_sha256: string;
  algo: string | null;
  size: number;
}

export interface CreateReleaseRequest {
  version: string;
  sha256: string;
}

export interface CreateDiffRequest {
  base_sha256: string;
  patch_sha256: string;
  algo?: string | null;
  size?: number;
}

// ---------- 资源（admin） ----------

export interface ResourceView {
  sha256: string;
  size: number;
  /** 可选显示名 */
  name: string | null;
  created_at: string;
  pools: string[];
}

export interface ResourceDetailView {
  sha256: string;
  size: number;
  name: string | null;
  created_at: string;
  pools: string[];
  /** 引用它的 app 名 */
  ref_apps: string[];
}

export interface UploadedView {
  sha256: string;
  size: number;
  name: string | null;
}

/** 池成员（池内对象） */
export interface PoolResourceView {
  sha256: string;
  name: string | null;
  size: number;
  added_at: string;
}

export interface AppResourceView {
  sha256: string;
  size: number;
}

// ---------- 公告 ----------

export interface AnnounceView {
  guid: string;
  title: string;
  content: string;
  /** 展示起点，null 不限 */
  starts_at: string | null;
  /** 展示终点，null 不限 */
  expires_at: string | null;
  /** permanent / scheduled / active / expired */
  status: string;
  created_at: string;
  updated_at: string;
  /** 引用这条公告的 app 名 */
  ref_apps: string[];
}

export interface CreateAnnounceRequest {
  title: string;
  content: string;
  starts_at?: string | null;
  expires_at?: string | null;
}

/** PATCH 全量语义：四个字段一次给全，null 表示清掉有效期 */
export interface UpdateAnnounceRequest {
  title: string;
  content: string;
  starts_at: string | null;
  expires_at: string | null;
}

// ---------- 客户端视角（X-App-Id） ----------

export interface ChannelView {
  guid: string;
  tag_name: string;
  latest_version: string;
  raw_sha256: string;
  raw_size: number;
  is_default: boolean;
}

export interface UpdateView {
  guid: string;
  tag_name: string;
  version: string;
  raw_sha256: string;
  raw_size: number;
  diffs: {
    base_sha256: string;
    patch_sha256: string;
    algo: string | null;
    size: number;
  }[];
}

export interface ClientAnnounceView {
  guid: string;
  title: string;
  content: string;
  /** 展示起点，null 不限 */
  starts_at: string | null;
  /** 展示终点，null 不限 */
  expires_at: string | null;
  created_at: string;
  updated_at: string;
}

// ---------- 请求体 ----------

export interface CreateAppRequest {
  id?: string;
  name: string;
}

export interface UpdateAppRequest {
  name?: string;
  enabled?: boolean;
}

export interface CreatePoolRequest {
  id: string;
  kind: string;
  endpoint: string;
  public_endpoint: string;
  secret: string;
  config?: Record<string, unknown>;
  is_public?: boolean;
}

export interface UpdatePoolRequest {
  endpoint?: string;
  public_endpoint?: string;
  secret?: string;
  config?: Record<string, unknown>;
  is_public?: boolean;
}