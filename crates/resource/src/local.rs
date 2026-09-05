//! 本地目录池：服务端自己的完整副本（router 用 `LOCAL_RESOURCE_DIR` 指定）。
//!
//! 本地副本是内容的"最完全"真相：节点上线前资源先落本地，之后各资源池的
//! 成员与同步都以它为推送源。它不对客户端提供下载——对外下载只走资源池重定向。

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::{fs, io::AsyncRead};

use crate::{
    error::PoolError,
    pool::{Entry, RemotePool},
};

pub struct LocalPool {
    root: PathBuf,
}

impl LocalPool {
    /// 目录不存在就建；建不了说明挂载或权限有问题，直接失败。
    pub async fn connect(root: impl Into<PathBuf>) -> Result<Self, PoolError> {
        let pool = Self { root: root.into() };
        fs::create_dir_all(&pool.root).await?;
        Ok(pool)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 文件大小，不存在返回 `PoolError::NotFound`。
    pub async fn size_of(&self, path: &str) -> Result<u64, PoolError> {
        let metadata = tokio::fs::metadata(self.resolve(path)?).await?;
        Ok(metadata.len())
    }

    /// 池内相对路径 → 绝对路径；`.` 与 `..` 一律拒绝，防越界。
    fn resolve(&self, path: &str) -> Result<PathBuf, PoolError> {
        let relative = Path::new(path.trim_start_matches('/'));
        if relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(PoolError::Config(format!("path escapes pool root: {path}")));
        }

        Ok(self.root.join(relative))
    }
}

#[async_trait]
impl RemotePool for LocalPool {
    async fn list_dir(&self, path: &str) -> Result<Vec<Entry>, PoolError> {
        let dir = self.resolve(path)?;
        let mut reader = fs::read_dir(&dir).await?;
        let mut entries = Vec::new();

        while let Some(entry) = reader.next_entry().await? {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().to_string();

            entries.push(Entry {
                path: format!("{}/{}", path.trim_end_matches('/'), name),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
                modified: modified_at(&metadata),
                name,
            });
        }

        Ok(entries)
    }

    async fn delete_file(&self, path: &str) -> Result<(), PoolError> {
        let full = self.resolve(path)?;
        fs::remove_file(&full).await?;

        // 清掉空父目录（一级分片目录 / tmp），非空时忽略失败
        if let Some(parent) = full.parent()
            && parent != self.root
        {
            let _ = fs::remove_dir(parent).await;
        }
        Ok(())
    }

    async fn download_file(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, PoolError> {
        let file = fs::File::open(self.resolve(path)?).await?;
        Ok(Box::new(file))
    }

    async fn upload_file(
        &self,
        path: &str,
        reader: &mut (dyn AsyncRead + Send + Unpin),
    ) -> Result<u64, PoolError> {
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&full).await?;
        let written = tokio::io::copy(reader, &mut file).await?;
        Ok(written)
    }

    async fn create_dir(&self, path: &str) -> Result<(), PoolError> {
        fs::create_dir_all(self.resolve(path)?).await?;
        Ok(())
    }

    async fn delete_dir(&self, path: &str) -> Result<(), PoolError> {
        fs::remove_dir_all(self.resolve(path)?).await?;
        Ok(())
    }
}

fn modified_at(metadata: &std::fs::Metadata) -> Option<DateTime<Utc>> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| DateTime::from_timestamp(duration.as_secs() as i64, 0))
}
