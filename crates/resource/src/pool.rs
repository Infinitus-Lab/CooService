//! 资源池抽象：把 S3 / FTP 等远端服务统一成 `RemotePool` trait。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::io::AsyncRead;

use crate::error::PoolError;

/// 目录条目。
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    /// 相对池根的完整路径
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
}

/// 远端资源池。实现必须 `Send + Sync`：池会被放进 `DashMap` 并在请求间共享。
///
/// 下载返回 `AsyncRead` 而非 `Vec<u8>`，上传接 `AsyncRead`，大文件不落内存。
#[async_trait]
pub trait RemotePool: Send + Sync {
    /// 列目录。`path` 为空表示池根。
    async fn list_dir(&self, path: &str) -> Result<Vec<Entry>, PoolError>;

    async fn delete_file(&self, path: &str) -> Result<(), PoolError>;

    async fn download_file(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, PoolError>;

    /// 返回写入的字节数；S3 侧拿不到该值，返回 0。
    async fn upload_file(
        &self,
        path: &str,
        reader: &mut (dyn AsyncRead + Send + Unpin),
    ) -> Result<u64, PoolError>;

    async fn create_dir(&self, path: &str) -> Result<(), PoolError>;

    /// 删除文件夹，含内部所有内容。
    async fn delete_dir(&self, path: &str) -> Result<(), PoolError>;
}

/// 资源池注册表。`dyn RemotePool` 没有 `Debug`，手动实现只打印 id 列表。
#[derive(Default)]
pub struct ResourcePool {
    pools: DashMap<String, Arc<dyn RemotePool>>,
}

impl std::fmt::Debug for ResourcePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourcePool")
            .field(
                "pools",
                &self
                    .pools
                    .iter()
                    .map(|e| e.key().clone())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ResourcePool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, id: String, pool: Arc<dyn RemotePool>) {
        self.pools.insert(id, pool);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn RemotePool>> {
        self.pools.get(id).map(|p| p.value().clone())
    }

    pub fn remove(&self, id: &str) -> Option<Arc<dyn RemotePool>> {
        self.pools.remove(id).map(|(_, p)| p)
    }

    /// 清空全部连接，reload 重扫时用。
    pub fn clear(&self) {
        self.pools.clear();
    }

    /// 当前登记的池 id 列表。
    pub fn ids(&self) -> Vec<String> {
        self.pools.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.pools.len()
    }
}
