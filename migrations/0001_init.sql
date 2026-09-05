-- 初始 schema：app / 资源池 / 资源 / 应用资源引用 / 发版通道 / 版本 / 差分 / 公告 / 应用公告引用
--
-- 约定：
--   · sha256 一律 BYTEA 存储（32 字节 = sha256 摘要），API 层用 hex 字符串，SQL 里 decode/encode 转换
--   · 内容侧 FK：通道/版本/差分/app_resource/app_announce 引用 RESTRICT（被引用不可删）；
--   · resource_location.sha256 例外为 CASCADE——台账随资源级联，池内物理副本由路由层清
--   · 桥表（resource_location / app_resource / app_announce）主键 (主方, 从方) 天然防重
--   · 统计类状态（池健康 / 同步进度）保存在运行时内存，不入库

CREATE TABLE app (
    id         UUID PRIMARY KEY,
    name       TEXT NOT NULL,
    enabled    BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE resource_pool (
    id              TEXT PRIMARY KEY,   -- 运维自命名，如 coo / coo2
    kind            TEXT NOT NULL CHECK (kind IN ('s3', 'ftp')),
    endpoint        TEXT NOT NULL,      -- S3 终结点 URL / FTP 主机名
    public_endpoint TEXT NOT NULL,      -- 302 重定向目标
    secret          TEXT NOT NULL,      -- S3 secret key / FTP 密码
    config          JSONB NOT NULL DEFAULT '{}'
                    CHECK (jsonb_typeof(config) = 'object'),  -- kind 专属参数，列分工见 pools.rs
    is_public       BOOLEAN NOT NULL DEFAULT false,           -- 公开池参与下载 302
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (NOT is_public OR public_endpoint <> '')
);

-- 内容寻址：同一 sha256 跨 app、跨版本只存一份
CREATE TABLE resource (
    sha256     BYTEA PRIMARY KEY,
    size       BIGINT NOT NULL DEFAULT 0 CHECK (size >= 0),
    name       TEXT,   -- 可选显示名，资源自己的名字（全局，可改）
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX resource_created_idx ON resource(created_at DESC);

-- 资源 × 池 多对多：一份内容可在多个池存副本；下载只从 location 池里挑
CREATE TABLE resource_location (
    sha256     BYTEA NOT NULL REFERENCES resource(sha256) ON DELETE CASCADE,
    pool_id    TEXT NOT NULL REFERENCES resource_pool(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (sha256, pool_id)
);
CREATE INDEX resource_location_pool_idx ON resource_location(pool_id);

-- app 维度引用资源（纯引用，不单独命名；资源名在 resource 本体上）
CREATE TABLE app_resource (
    app_id     UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
    sha256     BYTEA NOT NULL REFERENCES resource(sha256) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app_id, sha256)
);
CREATE INDEX app_resource_sha_idx ON app_resource(sha256);

-- 发版通道：guid 对外引用，tag_name 同 app 内唯一
CREATE TABLE channel (
    guid           UUID PRIMARY KEY,
    app_id         UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
    tag_name       TEXT NOT NULL,
    latest_version TEXT NOT NULL,
    latest_sha256  BYTEA NOT NULL REFERENCES resource(sha256) ON DELETE RESTRICT,
    is_default     BOOLEAN NOT NULL DEFAULT false,   -- 客户端首次启动优先挑默认通道
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (app_id, tag_name)
);

-- 每个 app 最多一个默认通道；部分索引只约束 is_default 为真的行
CREATE UNIQUE INDEX channel_default_idx ON channel (app_id) WHERE is_default;
-- 客户端按 app 列通道（默认在前、随后按创建序），管理列表同方向
CREATE INDEX channel_app_created_idx ON channel(app_id, created_at DESC);

CREATE TABLE channel_release (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    channel_guid UUID NOT NULL REFERENCES channel(guid) ON DELETE CASCADE,
    version      TEXT NOT NULL,
    sha256       BYTEA NOT NULL REFERENCES resource(sha256) ON DELETE RESTRICT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (channel_guid, version)
);
CREATE INDEX channel_release_recent_idx ON channel_release(channel_guid, created_at DESC);

-- 一个基准版本一个补丁：base -> patch（客户端以本地 sha 比对命中）
CREATE TABLE channel_diff (
    channel_guid UUID NOT NULL REFERENCES channel(guid) ON DELETE CASCADE,
    base_sha256  BYTEA NOT NULL REFERENCES resource(sha256) ON DELETE RESTRICT,
    patch_sha256 BYTEA NOT NULL REFERENCES resource(sha256) ON DELETE RESTRICT,
    algo         TEXT,
    size         BIGINT NOT NULL DEFAULT 0 CHECK (size >= 0),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel_guid, base_sha256),
    CHECK (base_sha256 <> patch_sha256)
);

-- 公告：全局内容，与 app 解耦，通过 app_announce 引用（多 app 可共享同一条）
CREATE TABLE announce (
    guid       UUID PRIMARY KEY,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    starts_at  TIMESTAMPTZ,   -- 展示起点，NULL 不限
    expires_at TIMESTAMPTZ,   -- 展示终点，NULL 不限
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (starts_at IS NULL OR expires_at IS NULL OR expires_at > starts_at)
);
CREATE INDEX announce_created_idx ON announce(created_at DESC);

CREATE TABLE app_announce (
    app_id        UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
    announce_guid UUID NOT NULL REFERENCES announce(guid) ON DELETE RESTRICT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app_id, announce_guid)
);
CREATE INDEX app_announce_guid_idx ON app_announce(announce_guid);