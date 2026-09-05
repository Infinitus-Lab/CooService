//! 数据库配置，全部来自环境变量。

use std::{env, path::PathBuf, time::Duration};

const DEFAULT_MAX_CONNECTIONS: u32 = 10;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;
const DEFAULT_MIGRATIONS_DIR: &str = "migrations";

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// 连接串，环境变量 `DATABASE_URL`
    pub url: String,
    /// 连接池上限，环境变量 `DATABASE_MAX_CONNECTIONS`，默认 10
    pub max_connections: u32,
    /// 建连超时，环境变量 `DATABASE_CONNECT_TIMEOUT_SECS`，默认 5s
    pub connect_timeout: Duration,
    /// SQL 迁移文件目录，环境变量 `MIGRATIONS_DIR`，默认 `migrations`
    pub migrations_dir: PathBuf,
}

impl DatabaseConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let url =
            env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?;

        let max_connections =
            parse_env::<u32>("DATABASE_MAX_CONNECTIONS")?.unwrap_or(DEFAULT_MAX_CONNECTIONS);
        let connect_timeout = parse_env::<u64>("DATABASE_CONNECT_TIMEOUT_SECS")?
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_SECS);

        let migrations_dir = env::var("MIGRATIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_MIGRATIONS_DIR));

        Ok(Self {
            url,
            max_connections,
            connect_timeout: Duration::from_secs(connect_timeout),
            migrations_dir,
        })
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
