//! S3 兼容对象存储（AWS S3 / MinIO / R2 / B2 等）。
//!
//! S3 没有真正的目录，目录是实现细节：
//! - `create_dir` 写一个以 `/` 结尾的空对象作为标记
//! - `list_dir` 用 `delimiter = "/"` 把 `CommonPrefixes` 当目录返回
//! - `delete_dir` 递归列出前缀下所有 key 再批量删除

use std::io;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use s3::{bucket::Bucket, creds::Credentials, region::Region, serde_types::ObjectIdentifier};
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;

use crate::{
    error::PoolError,
    pool::{Entry, RemotePool},
};

/// S3 批量删除接口单次的 key 上限
const DELETE_BATCH: usize = 1000;

/// S3 兼容服务（AWS S3 / MinIO / R2 / B2）的连接配置。
pub struct S3Config {
    /// 终结点，如 `http://127.0.0.1:9000`
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// MinIO 等自建服务通常要求 path style
    pub path_style: bool,
}

/// 一个 S3 桶的连接，`Bucket` 内部自带 HTTP 客户端，可并发使用。
pub struct S3Pool {
    bucket: Box<Bucket>,
}

impl S3Pool {
    /// 建连并校验：列一次根目录，凭据或 endpoint 不对会在这里失败。
    pub async fn connect(config: S3Config) -> Result<Self, PoolError> {
        let region = Region::Custom {
            region: config.region,
            endpoint: config.endpoint,
        };
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| PoolError::Config(format!("invalid s3 credentials: {e}")))?;

        let bucket = Bucket::new(&config.bucket, region, credentials)?;
        let bucket = if config.path_style {
            bucket.with_path_style()
        } else {
            bucket
        };

        let pool = Self { bucket };
        pool.list_dir("").await?;
        Ok(pool)
    }
}

#[async_trait]
impl RemotePool for S3Pool {
    async fn list_dir(&self, path: &str) -> Result<Vec<Entry>, PoolError> {
        let prefix = dir_prefix(path);
        let mut entries = Vec::new();

        for page in self
            .bucket
            .list(prefix.clone(), Some("/".to_string()))
            .await?
        {
            for object in page.contents {
                // 跳过目录标记对象和前缀自身
                if object.key.ends_with('/') || object.key == prefix {
                    continue;
                }
                entries.push(Entry {
                    name: base_name(&object.key),
                    path: object.key.clone(),
                    is_dir: false,
                    size: object.size,
                    modified: parse_time(&object.last_modified),
                });
            }

            for common in page.common_prefixes.unwrap_or_default() {
                let path = common.prefix.trim_end_matches('/').to_string();
                entries.push(Entry {
                    name: base_name(&path),
                    path,
                    is_dir: true,
                    size: 0,
                    modified: None,
                });
            }
        }

        Ok(entries)
    }

    async fn delete_file(&self, path: &str) -> Result<(), PoolError> {
        self.bucket.delete_object(path).await?;
        Ok(())
    }

    async fn download_file(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, PoolError> {
        let response = self.bucket.get_object_stream(path).await?;
        let stream = response.bytes.map_err(io::Error::other);
        Ok(Box::new(StreamReader::new(stream)))
    }

    async fn upload_file(
        &self,
        path: &str,
        reader: &mut (dyn AsyncRead + Send + Unpin),
    ) -> Result<u64, PoolError> {
        self.bucket.put_object_stream(reader, path).await?;
        // rust-s3 不回传写入字节数，用 list 查一次代价太高，这里只能返回 0
        Ok(0)
    }

    async fn create_dir(&self, path: &str) -> Result<(), PoolError> {
        self.bucket.put_object(dir_prefix(path), &[]).await?;
        Ok(())
    }

    async fn delete_dir(&self, path: &str) -> Result<(), PoolError> {
        let keys: Vec<ObjectIdentifier> = self
            .bucket
            .list(dir_prefix(path), None)
            .await?
            .into_iter()
            .flat_map(|page| page.contents)
            .map(|object| ObjectIdentifier::new(object.key))
            .collect();

        for batch in keys.chunks(DELETE_BATCH) {
            self.bucket.delete_objects(batch.to_vec()).await?;
        }

        Ok(())
    }
}

fn dir_prefix(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    match trimmed {
        "" => String::new(),
        _ => format!("{trimmed}/"),
    }
}

fn base_name(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
