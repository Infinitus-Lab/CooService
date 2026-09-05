//! 服务配置，全部来自环境变量。

use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8081";
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_LOCAL_RESOURCE_DIR: &str = "/data/resource";
const DEFAULT_POOL_HEALTH_INTERVAL_SECS: u64 = 300;
const DEFAULT_POOL_HEALTH_TIMEOUT_SECS: u64 = 10;

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
}

impl ServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr: SocketAddr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid BIND_ADDR: {e}"))?;

        let request_timeout = env::var("REQUEST_TIMEOUT_SECS")
            .ok()
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|e| anyhow::anyhow!("invalid REQUEST_TIMEOUT_SECS: {e}"))
            })
            .transpose()?
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        let local_resource_dir = env::var("LOCAL_RESOURCE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_LOCAL_RESOURCE_DIR));

        let pool_health_interval = env::var("POOL_HEALTH_INTERVAL_SECS")
            .ok()
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|e| anyhow::anyhow!("invalid POOL_HEALTH_INTERVAL_SECS: {e}"))
            })
            .transpose()?
            .unwrap_or(DEFAULT_POOL_HEALTH_INTERVAL_SECS);

        let pool_health_timeout = env::var("POOL_HEALTH_TIMEOUT_SECS")
            .ok()
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|e| anyhow::anyhow!("invalid POOL_HEALTH_TIMEOUT_SECS: {e}"))
            })
            .transpose()?
            .unwrap_or(DEFAULT_POOL_HEALTH_TIMEOUT_SECS);

        Ok(Self {
            bind_addr,
            request_timeout: Duration::from_secs(request_timeout),
            local_resource_dir,
            pool_health_interval: Duration::from_secs(pool_health_interval),
            pool_health_timeout: Duration::from_secs(pool_health_timeout),
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
        }
    }
}
