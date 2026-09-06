import type {
  AdminChannelView,
  AdminDiffView,
  AnnounceView,
  AppResourceView,
  AppView,
  ChannelView,
  ClientAnnounceView,
  PoolResourceView,
  CreateAnnounceRequest,
  CreateAppRequest,
  CreateChannelRequest,
  CreateDiffRequest,
  CreatePoolRequest,
  CreateReleaseRequest,
  PoolView,
  ReleaseView,
  ResourceDetailView,
  ResourceView,
  UpdateAnnounceRequest,
  UpdateAppRequest,
  UpdateChannelRequest,
  UpdatePoolRequest,
  UpdateView,
  UploadedView,
} from './types';

/// 所有接口统一返回这个信封
interface Envelope<T> {
  code: number;
  message: string;
  data: T | null;
  timestamp: string;
}

export class ApiError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
  }
}

/// 服务不可用时代理可能返回非 JSON（如 HTML），解析失败当无信封处理
function parseEnvelope<T>(text: string): Envelope<T> | null {
  try {
    return JSON.parse(text) as Envelope<T>;
  } catch {
    return null;
  }
}

/// 后端错误信封是英文技术串，这里按语义前缀映射成中文业务提示，保留具体标识（sha/guid）。
function friendlyError(code: number, message: string): string {
  const head = message.split(':')[0]?.trim().toLowerCase() ?? '';
  const tail = message.includes(':') ? message.slice(message.indexOf(':') + 1).trim() : '';
  const suffix = tail ? `：${tail}` : '';

  switch (head) {
    case 'bad request':
      return `请求参数有误${suffix}`;
    case 'unauthorized':
      return '密钥无效或未提供';
    case 'forbidden':
      return '无权访问该资源';
    case 'not found':
      return `目标不存在${suffix}`;
    case 'conflict':
      if (message.toLowerCase().includes('already in pool')) return '该资源已在池内';
      if (message.toLowerCase().includes('referenced')) return '资源已被引用，无法删除';
      return `操作冲突${suffix}`;
    case 'bad gateway':
      return '上游存储服务暂不可用';
    case 'service unavailable':
      return '服务暂不可用（如无可用公开存储池）';
    case 'internal error':
      return '服务内部错误，请稍后重试';
    default:
      return message || `请求失败（${code}）`;
  }
}

class Client {
  constructor(
    /** API 基址；空串 = 同源（由 ApiSetupDialog 配置） */
    private readonly baseApi: string,
    private readonly adminKey: () => string,
    private readonly onUnauthorized: () => void,
    /** 地址不可达（fetch 失败 / 目标不是 CooService API）时回调，用于弹出设置框 */
    private readonly onUnreachable: () => void,
  ) {}

  private async finish<T>(response: Response, path: string): Promise<T> {
    const text = await response.text();
    const envelope = parseEnvelope<T>(text);

    if (!response.ok) {
      // 401 只来自管理接口，直接退回密钥门（首页不是登录页，是请求失败才弹出）
      if (response.status === 401) {
        if (path.startsWith('/api/v1/admin')) {
          this.onUnauthorized();
        }
        throw new ApiError(response.status, '密钥无效或未提供');
      }
      throw new ApiError(
        envelope?.code ?? response.status,
        friendlyError(envelope?.code ?? response.status, envelope?.message ?? ''),
      );
    }

    // 200 但非 JSON 信封：目标不是 CooService API（如 SPA 回退页/错误页），视作地址不可达
    if (!envelope) {
      this.onUnreachable();
      throw new ApiError(0, '返回的不是 CooService API 响应，请检查 API 地址');
    }

    return envelope?.data as T;
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    headers: Record<string, string> = {},
  ): Promise<T> {
    let response: Response;
    try {
      response = await fetch(`${this.baseApi}${path}`, {
        method,
        headers: {
          ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
          ...headers,
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch {
      this.onUnreachable();
      throw new ApiError(0, `连不上服务（${this.baseApi || '同源'}）`);
    }
    return this.finish<T>(response, path);
  }

  /// 裸 body 上传（资源文件），不设 Content-Type，不序列化
  private async raw<T>(method: string, path: string, body: Blob): Promise<T> {
    let response: Response;
    try {
      response = await fetch(`${this.baseApi}${path}`, {
        method,
        headers: { 'X-Admin-Key': this.adminKey() },
        body,
      });
    } catch {
      this.onUnreachable();
      throw new ApiError(0, `连不上服务（${this.baseApi || '同源'}）`);
    }
    return this.finish<T>(response, path);
  }

  /// 管理接口：带 X-Admin-Key
  private admin<T>(method: string, path: string, body?: unknown): Promise<T> {
    return this.request<T>(method, path, body, { 'X-Admin-Key': this.adminKey() });
  }

  /// 客户端接口：带 X-App-Id，不需要管理密钥
  private client<T>(method: string, path: string, appId: string): Promise<T> {
    return this.request<T>(method, path, undefined, { 'X-App-Id': appId });
  }

  reloadApps = () => this.admin<string>('POST', '/api/v1/admin/reload');

  // ---------- 应用 ----------

  listApps = () => this.admin<AppView[]>('GET', '/api/v1/admin/apps');

  createApp = (body: CreateAppRequest) =>
    this.admin<AppView>('POST', '/api/v1/admin/apps', body);

  updateApp = (id: string, body: UpdateAppRequest) =>
    this.admin<AppView>('PATCH', `/api/v1/admin/apps/${id}`, body);

  deleteApp = (id: string) => this.admin<string>('DELETE', `/api/v1/admin/apps/${id}`);

  // ---------- 资源池 ----------

  listPools = () => this.admin<PoolView[]>('GET', '/api/v1/admin/pools');

  createPool = (body: CreatePoolRequest) =>
    this.admin<PoolView>('POST', '/api/v1/admin/pools', body);

  updatePool = (id: string, body: UpdatePoolRequest) =>
    this.admin<PoolView>('PATCH', `/api/v1/admin/pools/${id}`, body);

  deletePool = (id: string) => this.admin<string>('DELETE', `/api/v1/admin/pools/${id}`);

  /// 强制全扫描：重建各池实有集并补齐缺失，返回各池状态
  scanPools = () => this.admin<Record<string, string>>('POST', '/api/v1/admin/pools/scan');

  // ---------- 发版通道（admin） ----------

  listChannelsAdmin = (appId?: string) => {
    const query = appId ? `?app_id=${encodeURIComponent(appId)}` : '';
    return this.admin<AdminChannelView[]>('GET', `/api/v1/admin/channels${query}`);
  };

  createChannel = (body: CreateChannelRequest) =>
    this.admin<AdminChannelView>('POST', '/api/v1/admin/channels', body);

  updateChannel = (guid: string, body: UpdateChannelRequest) =>
    this.admin<AdminChannelView>('PATCH', `/api/v1/admin/channels/${guid}`, body);

  deleteChannel = (guid: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/channels/${guid}`);

  listReleases = (guid: string) =>
    this.admin<ReleaseView[]>('GET', `/api/v1/admin/channels/${guid}/releases`);

  createRelease = (guid: string, body: CreateReleaseRequest) =>
    this.admin<ReleaseView>('POST', `/api/v1/admin/channels/${guid}/releases`, body);

  deleteRelease = (guid: string, version: string) =>
    this.admin<string>(
      'DELETE',
      `/api/v1/admin/channels/${guid}/releases/${encodeURIComponent(version)}`,
    );

  listDiffs = (guid: string) =>
    this.admin<AdminDiffView[]>('GET', `/api/v1/admin/channels/${guid}/diffs`);

  createDiff = (guid: string, body: CreateDiffRequest) =>
    this.admin<AdminDiffView>('POST', `/api/v1/admin/channels/${guid}/diffs`, body);

  deleteDiff = (guid: string, baseSha256: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/channels/${guid}/diffs/${baseSha256}`);

  // ---------- 资源（admin） ----------

  listResources = (pool?: string, sha256?: string) => {
    const params = new URLSearchParams();
    if (pool) params.set('pool', pool);
    if (sha256) params.set('sha256', sha256);
    const query = params.toString();
    return this.admin<ResourceView[]>('GET', `/api/v1/admin/resources${query ? `?${query}` : ''}`);
  };

  /// 上传资源到本地库（始终先写本地完整副本；进池由资源池管理负责）
  uploadResource = (file: Blob, name?: string) => {
    const query = name?.trim() ? `?name=${encodeURIComponent(name.trim())}` : '';
    return this.raw<UploadedView>('PUT', `/api/v1/admin/resources${query}`, file);
  };

  getResourceDetail = (sha256: string) =>
    this.admin<ResourceDetailView>('GET', `/api/v1/admin/resources/${sha256}`);

  deleteResource = (sha256: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/resources/${sha256}`);

  /// 资源池成员管理（池的领域）
  poolResources = (poolId: string) =>
    this.admin<PoolResourceView[]>('GET', `/api/v1/admin/pools/${poolId}/resources`);

  poolAddResource = (poolId: string, sha256: string) =>
    this.admin<PoolResourceView>('POST', `/api/v1/admin/pools/${poolId}/resources`, { sha256 });

  poolRemoveResource = (poolId: string, sha256: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/pools/${poolId}/resources/${sha256}`);

  listAppResources = (appId: string) =>
    this.admin<AppResourceView[]>('GET', `/api/v1/admin/apps/${appId}/resources`);

  linkAppResource = (appId: string, sha256: string) =>
    this.admin<AppResourceView>('POST', `/api/v1/admin/apps/${appId}/resources`, { sha256 });

  /// 修改资源显示名（资源自己的名字，全局；空串清空）
  renameResource = (sha256: string, name?: string) =>
    this.admin<ResourceDetailView>('PATCH', `/api/v1/admin/resources/${sha256}`, { name });

  unlinkAppResource = (appId: string, sha256: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/apps/${appId}/resources/${sha256}`);

  // ---------- 公告 ----------

  listAnnounces = (appId?: string) => {
    const query = appId ? `?app_id=${encodeURIComponent(appId)}` : '';
    return this.admin<AnnounceView[]>('GET', `/api/v1/admin/announces${query}`);
  };

  createAnnounce = (body: CreateAnnounceRequest) =>
    this.admin<AnnounceView>('POST', '/api/v1/admin/announces', body);

  updateAnnounce = (guid: string, body: UpdateAnnounceRequest) =>
    this.admin<AnnounceView>('PATCH', `/api/v1/admin/announces/${guid}`, body);

  deleteAnnounce = (guid: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/announces/${guid}`);

  /// app 引用公告（公告全局一份，多 app 可共享）
  listAppAnnounces = (appId: string) =>
    this.admin<AnnounceView[]>('GET', `/api/v1/admin/apps/${appId}/announces`);

  linkAppAnnounce = (appId: string, guid: string) =>
    this.admin<AnnounceView>('POST', `/api/v1/admin/apps/${appId}/announces`, { guid });

  unlinkAppAnnounce = (appId: string, guid: string) =>
    this.admin<string>('DELETE', `/api/v1/admin/apps/${appId}/announces/${guid}`);

  // ---------- 客户端视角（X-App-Id，只读预览用） ----------

  listChannels = (appId: string) =>
    this.client<ChannelView[]>('GET', '/api/v1/app/channels', appId);

  getUpdate = (guid: string) =>
    this.request<UpdateView>('GET', `/api/v1/app/channels/${guid}`);

  listClientAnnounces = (appId: string) =>
    this.client<ClientAnnounceView[]>('GET', '/api/v1/app/announces', appId);
}

export const createClient = (
  baseApi: string,
  adminKey: () => string,
  onUnauthorized: () => void,
  onUnreachable: () => void,
) => new Client(baseApi, adminKey, onUnauthorized, onUnreachable);

export type ApiClient = Client;