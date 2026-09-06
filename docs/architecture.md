# CooService 架构文档

更新分发服务（游戏/应用更新）：登记应用 → 发版通道（版本/差分）→ 客户端按本地 sha256 拉取
完整包或补丁。存储抽象（S3/FTP）+ 本地完整副本双轨，管理端 WebUI + 客户端只读 API。

## 1. 分层

```
crates/
  database/   PostgreSQL 访问层（tokio-postgres + deadpool）
              · 连接池 / 迁移 / repo 按表组织，参数一律 $n 占位
  apps/       应用领域层：AppManager（DB 为唯一真相，内存只是缓存）
  resource/   存储抽象（RemotePool trait：S3 / FTP / 本地目录）
              · 只做存储，不碰数据库（铁律）
              · 含池健康检查（存活性探测是存储抽象自己的职责）
  router/     axum 路由层：鉴权 / 信封 / 路由树 / 池同步引擎（调度者）
```

依赖方向：`router → {apps, resource, database}`，`apps → database`，`resource` 无下游。

## 2. 领域模型

| 领域 | 载体 | 说明 |
|---|---|---|
| 应用 | `app` | 客户端身份（`X-App-Id`），启停开关 |
| 资源 | `resource` | **全局内容寻址**：sha256 为 PK，与 app 解耦 |
| 资源池 | `resource_pool` | 外部 S3/FTP 配置，`is_public` 决定是否参与下载重定向 |
| 池成员 | `resource_location` | 资源 × 池多对多：一份内容可在多池存副本 |
| 应用资源引用 | `app_resource` | app 引用全局资源（可选别名 `alias`，app 内唯一） |
| 发版通道 | `channel` | 每 app 多通道，`is_default` 部分唯一索引保证 app 内唯一默认 |
| 版本记录 | `channel_release` | 通道历史版本（内容 sha256，upsert） |
| 差分 | `channel_diff` | `base_sha256 → patch_sha256`（同通道内，客户端按本地 sha 命中） |
| 公告 | `announce` | 全局内容，`[starts_at, expires_at)` 展示有效期（NULL = 不限） |
| 应用公告引用 | `app_announce` | 多 app 共享同一条公告 |

**内容侧 FK**：通道/版本/差分/app_resource/app_announce 引用 RESTRICT——被引用不可删、
删除返回 409；`resource_location.sha256` 例外为 CASCADE（台账随资源级联，池内物理副本由路由层清）。
桥表主键 `(主方, 从方)` 天然防重。

## 3. 内容寻址与下载规则

- 上传永远写**本地完整副本**（`LOCAL_RESOURCE_DIR`，tmp → sha256 边写边算 → rename 原子）
  → 插 `resource` 行。可带可选显示名 `name`。
- **服务端不代理流转发**。下载只有一条 200 路径：

| 场景 | 响应 |
|---|---|
| 已连接池数 = 0 | 503 无可用存储池 |
| 资源无 location（仅本地） | 503 未入库任何池 |
| 有 location 但无公开池（`is_public` ∧ 已连接 ∧ 健康 up） | 503 无公开存储池 |
| 有公开候选 | 302 随机重定向到 `public_endpoint/{分片key}`（唯一 200） |

- 分片 key：`sha256` 前两位做一级目录 `ab/cdef...`。

## 4. 资源池

### 4.1 列分工（`resource_pool`）

| 列 | S3 | FTP |
|---|---|---|
| `endpoint` | 终结点 URL | 主机名 |
| `public_endpoint` | 对外下载链接（302 目标） | 同左 |
| `secret` | secret key | 密码 |
| `config` | `{bucket, region, access_key, path_style}` | `{user, root, port, secure}` |

`config` 按 kind 校验（s3 缺 bucket/region/access_key、ftp 缺 user → 400），DB CHECK
要求 JSON 对象、`is_public` 必须有非空 `public_endpoint`；`public_endpoint` 必须是
http/https URL（`routes/admin/pools.rs` 校验）。FTP `secure: true` 走显式 FTPS
（AUTH TLS，凭据与内容加密过网）；自建 FTPS 自签证书可加 `secure_skip_verify: true`
（生产公网不建议）。

### 4.2 在线情况（resource crate）

`crates/resource/src/health.rs`：`PoolHealth` 监督任务每周期对注册池做 `list_dir("")`
探测（`POOL_HEALTH_INTERVAL_SECS` 默认 300s / `POOL_HEALTH_TIMEOUT_SECS` 10s），
新注册池立即探测。`connected`（建连成功）与 `up`（探测通过）两层；
探测逻辑在存储抽象层，router 只调度与读状态。下载候选要求健康 `up`。

### 4.3 池成员管理（router）

- `GET /admin/pools/{id}/resources` 池内对象列表
- `POST /admin/pools/{id}/resources {"sha256"}` 本地资源入池：本地副本推池 → 登记 location
- `DELETE /admin/pools/{id}/resources/{sha256}` 池内移除（不碰本地；删远端对象并注销 location）

**领域边界**：资源管理只管本地库（上传/列表/详情/删除）；进池、同步是资源池的事。

### 4.4 同步引擎（router，`pool_sync.rs`）

模型：**本地完整副本 = 真相**；`resource_location` = 每池期望集；实有集 = 启动全扫描
（按分片目录递归列出）+ 本服务写成功乐观更新（`record_write`）。

- 缺失 = 期望 − 实有 → **自动从本地推池补齐**（本地缺则记 warn，不算操作数）
- 启动后台全扫不阻塞 serve；变动事件（上传/入池/移除/删除/reload）触发增量对账
- 管理接口 `POST /admin/pools/scan` 强制全扫描
- 每池状态：`syncing(pending) / synced / error`
- 引擎只补缺失、不清理池内多余对象（实有 ⊇ 期望）：外部手工放入池的杂物不会被删

## 5. 发版流水线与界面边界

写版本 + 生成差分是独立流水线，**不进管理界面**；界面只做通道/版本记录/差分 CRUD
（资源先于通道存在——`latest_sha256` 必须指向已登记资源，先查后插给 400/404）。

## 6. API 约定

- 统一信封 `{code, message, data, timestamp}`；错误 `AppError` 语义化映射
  （400/401/404/409/502/503，`DatabaseError::Constraint` 23503/23505 → 409）
- 管理接口：`/api/v1/admin` 整树 `X-Admin-Key`。密钥来源：`ADMIN_KEY` 环境变量
  （运维密管，`openssl rand -hex 64` 生成后注入）；未注入时每次启动随机生成且
  **不打印完整密钥**（日志只有前 8 位指纹），此时无法从日志取回，正式部署必须显式注入。
  无登录页，401 才弹密钥门
- 客户端接口：通道/公告需 `X-App-Id`（只读）；更新信息按 guid 即可取
- 管理路由统一放 `routes/admin/` 目录（目录头注释声明领域边界），
  公开接口在 `routes/{app,resources}.rs`，**不混写**
- 迁移：`migrations/*.sql` 按序执行，`_migrations` 记录；alpha 已合并为单一
  `0001_init.sql`，后续从 `0002_` 递增
- 路径无尾斜杠：`/api/v1/app/channels/`、`/announces/` 这种带 `/` 的请求 404
  （axum 不做斜杠重定向）；`/admin/apps/` 因 nest+`/` 反而兼容——客户端拼 URL 勿加尾斜杠
- admin key 取法：`ADMIN_KEY=$(openssl rand -hex 64)` 写入宿主 .env 后
  `docker compose up -d`，WebUI 密钥门粘贴同一值；日志只核对前 8 位指纹。

## 7. 部署拓扑

```
浏览器 ─ 8082 ─► [caddy]（coo-web，静态 WebUI，SPA 回退）
游戏客户端 ─ 8081 ─► [coo-router]（coo-api，直连）
coo-web（caddy + anubis 预留）/ coo-api（pgsql + router）两个 internal 网络，完全隔离无桥接
```

- `compose.yml`：服务结构 + 固定配置；部署环境变量在 `compose.override.yml` 实例化
  （compose 自动合并加载），敏感值从宿主 `.env` 注入（模板 `.env.example`）。
  `coo-pgsql`（不发布端口）+ `coo-router`（127.0.0.1:8081
  + `./data/resource:/data/resource:U,z` 本地副本，启动时校验可写）+ 临时直出 caddy
  （anubis 待 cloudflared IP 方案恢复）——见 compose 注释
- WebUI 构建期注入 `VITE_BASE_API`（默认 `http://127.0.0.1:8081`）；跨域由
  router 的 permissive CORS 放行
- podman 坑：`up -d` 不因镜像重建而重建容器（需 `--force-recreate`）；
- 卷重建：改 `POSTGRES_PASSWORD` 后若 `pgdata` 卷还在则密码不生效，需先
  `podman volume rm pgdata`（或 `down -v`）再 `up -d`；`data/resource` 是资源唯一真相，删卷即丢全部资源
  bind 挂载受 SELinux 管（容器内交互用 stdin/`--entrypoint`），测试产物只放 `data/`

## 8. 环境变量

| 变量 | 默认 | 用途 |
|---|---|---|
| `DATABASE_URL` | 必填 | 连接串 |
| `DATABASE_MAX_CONNECTIONS` | 10 | 连接池上限 |
| `DATABASE_CONNECT_TIMEOUT_SECS` | 5 | 建连超时 |
| `MIGRATIONS_DIR` | `migrations` | 迁移目录 |
| `BIND_ADDR` | `127.0.0.1:8081` | 监听地址 |
| `REQUEST_TIMEOUT_SECS` | 30 | 请求超时（全树统一，上传分支豁免，见 `router.rs`） |
| `LOCAL_RESOURCE_DIR` | `/data/resource` | 本地完整副本 |
| `POOL_HEALTH_INTERVAL_SECS` | 300 | 池健康探测周期 |
| `POOL_HEALTH_TIMEOUT_SECS` | 10 | 单次探测超时 |
| `ADMIN_KEY` | 随机生成 | 管理密钥；未注入则随机且不打印，见上 |
| `STORAGE_SECRET_MASTER_KEY` | 无 | 池凭据 AES-256 主密钥（64 位 hex）；不配置则明文落库并告警 |
| `MAX_UPLOAD_BYTES` | 2 GiB | 上传请求体上限（超出 413，慢链路不受请求超时约束） |
| `PUBLIC_RATE_LIMIT` | 120 | 公开端点每 IP 每分钟限流 |
| `TRUST_X_FORWARDED_FOR` | 0 | 挂可信反代后置 1：限流按 XFF 计客户端 IP（否则共享代理 IP 全局限流） |
| `CORS_ALLOWED_ORIGINS` | 空（放行全部） | 逗号分隔的 Origin 白名单 |
| `RUST_LOG` | info | 日志级别 |

## 9. 前端（WebUI）

- daisyUI 5 常规配色：`light` 为默认、`dark` 跟随系统 `prefers-color-scheme`；
  无渐变；表单控件槽样走 `@utility` 覆盖（明暗主题下都保持可读）
- 页面：顶层 应用/资源池/资源/公告 + App 详情左侧栏（概览/通道/资源/公告，
  按关联过滤）；密钥门 401 驱动；哈希一律 HashChip（可复制指纹）
- 构建：`pnpm typecheck && pnpm build`（Tailwind v4 + daisyUI 5）

## 10. 已知取舍（上线前复核）

- `resource_pool.secret` 已支持 AES-256 静态加密（`STORAGE_SECRET_MASTER_KEY`）；
  未配置主密钥时仍明文落库（兼容旧部署），生产必须配置
- 管理列表无分页（alpha 量级；`channel(app_id, is_default, created_at)` 组合索引待需再补）
- anubis 的 `X-Real-Ip` 来源待 cloudflared 方案（当前 caddy 临时直出 8082）
- 公开端点限流是进程内固定窗口（每 IP 每分），精细限流 / 防分布式攻击仍应交网关

## 11. 注释与代码规范

- 注释解释**为什么**（不变量、时序约束、领域边界、外部系统的怪癖），
  不为"是什么"写注释——代码本身自明
- 不留过程叙事（"之前……后来""顺手""修复了"），不写贬褒，不贴日志
- 领域名词全局一致：资源/资源池/入池出池/同步/健康；不用"补副本/兜底/流式"等
  已废弃说法
- 模块头（`//!`）写明边界与关键模型，一处讲透，不在函数里复述第二遍
- 迁移文件与 SQL 注释同样适用；注释与实际行为脱节属于缺陷，随代码改动同步更新
- 后端 `clippy` 零警告、前端 `tsc --noEmit` 通过为提交前置条件；测试产物只落 `data/`