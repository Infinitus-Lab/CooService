//! 服务配置，全部来自环境变量。

use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

use crate::auth::{generate_admin_key, hex_encode};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8081";
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_LOCAL_RESOURCE_DIR: &str = "/data/resource";
const DEFAULT_POOL_HEALTH_INTERVAL_SECS: u64 = 300;
const DEFAULT_POOL_HEALTH_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB
const DEFAULT_PUBLIC_RATE_LIMIT: u32 = 120; // 每 IP 每分钟请求数

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// 监听地址，环境变量 `BIND_ADDR`，默认 `127.0.0.1:8081`
    pub bind_addr: SocketAddr,
    /// 单个请求的最长处理时间，环境变量 `REQUEST_TIMEOUT_SECS`，默认 30s
    pub request_timeout: Duration,
    /// 本地完整副本目录，环境变量 `LOCAL_RESOURCE_DIR`，默认 `/data/resource`
    pub local_resource_dir: PathBuf,
    /// 池健康探测周期，环境变量 `POOL_HEALTH_INTERVAL_SECS`，默认 300s
    pub pool_health_interval: Duration,
    /// 单次池探测超时，环境变量 `POOL_HEALTH_TIMEOUT_SECS`，默认 10s
    pub pool_health_timeout: Duration,
    /// 上传请求体上限，环境变量 `MAX_UPLOAD_BYTES`，默认 2 GiB
    pub max_upload_bytes: u64,
    /// 公开端点每 IP 限流（次/分钟），环境变量 `PUBLIC_RATE_LIMIT`，默认 120
    pub public_rate_limit: u32,
    /// 是否信任 `X-Forwarded-For`（可信反代后部署时开），环境变量 `TRUST_X_FORWARDED_FOR`
    pub trust_x_forwarded_for: bool,
    /// 管理密钥：`ADMIN_KEY` 未设置时随机生成（不可恢复，见 `auth` 模块头）
    pub admin_key: String,
    /// CORS 允许的 Origin 白名单（逗号分隔）；为空时放行全部（仅限内网开发）
    pub cors_allowed_origins: Vec<String>,
}

impl ServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr: SocketAddr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid BIND_ADDR: {e}"))?;

        let request_timeout =
            parse_env::<u64>("REQUEST_TIMEOUT_SECS")?.unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);
        let local_resource_dir = env::var("LOCAL_RESOURCE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_LOCAL_RESOURCE_DIR));

        let pool_health_interval = parse_env::<u64>("POOL_HEALTH_INTERVAL_SECS")?
            .unwrap_or(DEFAULT_POOL_HEALTH_INTERVAL_SECS);
        let pool_health_timeout = parse_env::<u64>("POOL_HEALTH_TIMEOUT_SECS")?
            .unwrap_or(DEFAULT_POOL_HEALTH_TIMEOUT_SECS);

        let max_upload_bytes =
            parse_env::<u64>("MAX_UPLOAD_BYTES")?.unwrap_or(DEFAULT_MAX_UPLOAD_BYTES);
        let public_rate_limit =
            parse_env::<u32>("PUBLIC_RATE_LIMIT")?.unwrap_or(DEFAULT_PUBLIC_RATE_LIMIT);
        let trust_x_forwarded_for = env::var("TRUST_X_FORWARDED_FOR")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

        let admin_key = match env::var("ADMIN_KEY").ok().filter(|s| !s.is_empty()) {
            Some(key) => key,
            None => {
                let key = generate_admin_key();
                tracing::warn!(
                    "ADMIN_KEY not set, generated an ephemeral admin key (not recoverable); set ADMIN_KEY to persist access"
                );
                key
            }
        };

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            bind_addr,
            request_timeout: Duration::from_secs(request_timeout),
            local_resource_dir,
            pool_health_interval: Duration::from_secs(pool_health_interval),
            pool_health_timeout: Duration::from_secs(pool_health_timeout),
            max_upload_bytes,
            public_rate_limit,
            trust_x_forwarded_for,
            admin_key,
            cors_allowed_origins,
        })
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_ADDR
                .parse()
                .expect("default bind addr is valid"),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            local_resource_dir: PathBuf::from(DEFAULT_LOCAL_RESOURCE_DIR),
            pool_health_interval: Duration::from_secs(DEFAULT_POOL_HEALTH_INTERVAL_SECS),
            pool_health_timeout: Duration::from_secs(DEFAULT_POOL_HEALTH_TIMEOUT_SECS),
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            public_rate_limit: DEFAULT_PUBLIC_RATE_LIMIT,
            trust_x_forwarded_for: false,
            admin_key: hex_encode(&[0u8; 64]), // 仅供测试构造；真实密钥走 ADMIN_KEY 或随机生成
            cors_allowed_origins: Vec::new(),
        }
    }
}

/// 变量不存在返回 `Ok(None)`，存在但解析失败返回 `Err`。
fn parse_env<T: std::str::FromStr>(key: &str) -> anyhow::Result<Option<T>>
where
    T::Err: std::fmt::Display,
{
    env::var(key)
        .ok()
        .map(|v| {
            v.parse::<T>()
                .map_err(|e| anyhow::anyhow!("invalid {key}: {e}"))
        })
        .transpose()
}
