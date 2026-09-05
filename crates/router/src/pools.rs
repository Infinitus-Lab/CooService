//! 资源池初始化：从 `resource_pool` 表读配置，建出真正的 S3 / FTP 连接。
//!
//! `resource` crate 只提供存储抽象，不碰数据库；列与 config 的分工在这里解释：
//!
//! | 列 | 含义 |
//! |---|---|
//! | `endpoint` | S3 是终结点 URL，FTP 是主机名（不含端口） |
//! | `public_endpoint` | 生成对外下载链接用 |
//! | `secret` | S3 是 secret key，FTP 是密码 |
//! | `config` | S3：`bucket` / `region` / `access_key` / `path_style`；FTP：`user` / `root` / `port` |
//! | `is_public` | true 时该池才会被选为下载 302 目标 |

use std::sync::Arc;

use database::{Database, repo};
use resource::{
    ftp::{FtpConfig, FtpPool},
    pool::{RemotePool, ResourcePool},
    s3::{S3Config, S3Pool},
};

use crate::state::{PoolMeta, PoolMetaMap};

/// 建全部池 + 公开信息表。单个池连不上只记错误并跳过，不拖垮启动。
pub async fn load_resource_pools(db: &Database) -> anyhow::Result<(ResourcePool, PoolMetaMap)> {
    let registry = ResourcePool::new();
    let meta = PoolMetaMap::new();
    connect_all(&registry, &meta, db).await?;
    Ok((registry, meta))
}

/// `/admin/reload` 用：清空后重新建连，池配置改动热生效。
pub async fn reload_resource_pools(
    registry: &ResourcePool,
    meta: &PoolMetaMap,
    db: &Database,
) -> anyhow::Result<()> {
    registry.clear();
    meta.clear();
    connect_all(registry, meta, db).await
}

async fn connect_all(
    registry: &ResourcePool,
    meta: &PoolMetaMap,
    db: &Database,
) -> anyhow::Result<()> {
    for row in repo::pool::list(db).await? {
        let pool: Result<Arc<dyn RemotePool>, String> = match row.kind.as_str() {
            "s3" => S3Pool::connect(S3Config {
                endpoint: row.endpoint.clone(),
                region: string(&row, "region"),
                bucket: string(&row, "bucket"),
                access_key: string(&row, "access_key"),
                secret_key: row.secret.clone(),
                path_style: bool_of(&row, "path_style"),
            })
            .await
            .map(|pool| Arc::new(pool) as Arc<dyn RemotePool>)
            .map_err(|e| e.to_string()),

            "ftp" => FtpPool::connect(FtpConfig {
                host: row.endpoint.clone(),
                port: u16::try_from(number(&row, "port")).unwrap_or(21),
                user: string(&row, "user"),
                password: row.secret.clone(),
                root: string(&row, "root"),
            })
            .await
            .map(|pool| Arc::new(pool) as Arc<dyn RemotePool>)
            .map_err(|e| e.to_string()),

            other => {
                tracing::warn!(pool = %row.id, kind = %other, "unknown pool kind, skipped");
                continue;
            }
        };

        match pool {
            Ok(pool) => {
                tracing::info!(pool = %row.id, kind = %row.kind, "resource pool connected");
                registry.register(row.id.clone(), pool);
                meta.insert(
                    row.id.clone(),
                    PoolMeta {
                        public_endpoint: row.public_endpoint.clone(),
                        is_public: row.is_public,
                    },
                );
            }
            Err(e) => {
                tracing::error!(pool = %row.id, error = %e, "resource pool connect failed, skipped")
            }
        }
    }

    tracing::info!(count = registry.count(), "resource pools loaded");
    Ok(())
}

fn string(row: &repo::pool::PoolRow, key: &str) -> String {
    row.config
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn number(row: &repo::pool::PoolRow, key: &str) -> u64 {
    row.config
        .get(key)
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
}

fn bool_of(row: &repo::pool::PoolRow, key: &str) -> bool {
    row.config
        .get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}
