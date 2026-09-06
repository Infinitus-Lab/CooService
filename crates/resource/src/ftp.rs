//! FTP 资源池。
//!
//! FTP 一条控制连接同时只能有一个传输在飞，所以连接放在 `Mutex` 里串行化。
//! 下载要 `retr_as_stream` -> 拷数据 -> `finalize_retr_stream` 三步连着做完，
//! 因此拷贝放后台任务，调用方拿 duplex 的读端；连接锁（`OwnedMutexGuard`）
//! 随任务一起移交，传输期间别的命令不会插进来。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use suppaftp::list::{ListParser, ParseResult};
use suppaftp::tokio::{AsyncNativeTlsConnector, AsyncNativeTlsFtpStream};
use suppaftp::types::FileType;
use tokio::io::{AsyncRead, AsyncWriteExt, duplex};
use tokio::sync::Mutex;

use crate::{
    error::PoolError,
    pool::{Entry, RemotePool},
};

const TRANSFER_BUFFER: usize = 64 * 1024;

/// FTP 连接配置。
pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// 池根路径，所有操作相对它
    pub root: String,
    /// true 时用显式 FTPS（AUTH TLS），凭据与内容都走加密通道
    pub secure: bool,
    /// 仅配合 `secure`：跳过证书校验（自建 FTPS 用自签证书时开；生产公网不建议）
    pub insecure_skip_verify: bool,
}

/// 一个 FTP 池：整池共用一条控制连接，操作串行化。
///
/// 流类型固定用 TLS 变体（`AsyncNativeTlsFtpStream`）：not secure 时它只是普通 TCP
/// 传输（suppaftp 只在 `into_secure` 后才走 TLS），这样 secure 开关不改变结构体类型。
pub struct FtpPool {
    root: String,
    stream: Arc<Mutex<AsyncNativeTlsFtpStream>>,
}

impl FtpPool {
    /// 建连、登录、切二进制，并校验一次根目录列表。
    pub async fn connect(config: FtpConfig) -> Result<Self, PoolError> {
        let mut stream = AsyncNativeTlsFtpStream::connect((config.host.as_str(), config.port)).await?;
        if config.secure {
            let connector = AsyncNativeTlsConnector::from(
                suppaftp::async_native_tls::TlsConnector::new()
                    .danger_accept_invalid_certs(config.insecure_skip_verify),
            );
            stream = stream.into_secure(connector, &config.host).await?;
        }
        stream.login(&config.user, &config.password).await?;
        stream.transfer_type(FileType::Binary).await?;

        let pool = Self {
            root: config.root,
            stream: Arc::new(Mutex::new(stream)),
        };
        pool.list_dir("").await?;
        Ok(pool)
    }

    fn full_path(&self, path: &str) -> String {
        let root = self.root.trim_matches('/');
        let path = path.trim_matches('/');

        match (root, path) {
            ("", "") => "/".to_string(),
            ("", path) => format!("/{path}"),
            (root, "") => format!("/{root}"),
            (root, path) => format!("/{root}/{path}"),
        }
    }
}

#[async_trait]
impl RemotePool for FtpPool {
    async fn list_dir(&self, path: &str) -> Result<Vec<Entry>, PoolError> {
        let target = self.full_path(path);
        let lines = self.stream.lock().await.list(Some(&target)).await?;

        let mut entries = Vec::new();
        for line in lines {
            let file = parse_line(&line)?;
            // LIST 只给裸文件名，路径要自己拼
            let path = format!("{}/{}", target.trim_end_matches('/'), file.name());
            entries.push(Entry {
                name: file.name().to_string(),
                path,
                is_dir: file.is_directory(),
                size: file.size() as u64,
                modified: system_time_to_utc(file.modified()),
            });
        }

        Ok(entries)
    }

    async fn delete_file(&self, path: &str) -> Result<(), PoolError> {
        self.stream.lock().await.rm(&self.full_path(path)).await?;
        Ok(())
    }

    async fn download_file(
        &self,
        path: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, PoolError> {
        let stream = Arc::clone(&self.stream);
        let target = self.full_path(path);

        // 先拿到锁再开传输，连接错误在这里就返回给调用方
        let mut guard = stream.lock_owned().await;
        let mut data = guard.retr_as_stream(&target).await?;

        let (reader, mut writer) = duplex(TRANSFER_BUFFER);
        tokio::spawn(async move {
            if let Err(e) = tokio::io::copy(&mut data, &mut writer).await {
                tracing::error!(error = %e, path = %target, "ftp copy failed");
            }
            if let Err(e) = guard.finalize_retr_stream(data).await {
                tracing::error!(error = %e, path = %target, "ftp finalize failed");
            }
        });

        Ok(Box::new(reader))
    }

    async fn upload_file(
        &self,
        path: &str,
        reader: &mut (dyn AsyncRead + Send + Unpin),
    ) -> Result<u64, PoolError> {
        let mut guard = self.stream.lock().await;
        let target = self.full_path(path);

        let mut data = guard.put_with_stream(&target).await?;
        let copied = tokio::io::copy(reader, &mut data).await?;
        data.flush().await?;
        guard.finalize_put_stream(data).await?;

        Ok(copied)
    }

    async fn create_dir(&self, path: &str) -> Result<(), PoolError> {
        self.stream
            .lock()
            .await
            .mkdir(&self.full_path(path))
            .await?;
        Ok(())
    }

    async fn delete_dir(&self, path: &str) -> Result<(), PoolError> {
        self.delete_dir_recursive(path).await
    }
}

impl FtpPool {
    /// FTP 的 RMD 只能删空目录，所以自底向上清空再删。
    async fn delete_dir_recursive(&self, path: &str) -> Result<(), PoolError> {
        for entry in self.list_dir(path).await? {
            if entry.is_dir {
                Box::pin(self.delete_dir_recursive(&trim_root(&entry.path))).await?;
            } else {
                Box::pin(self.delete_file(&trim_root(&entry.path))).await?;
            }
        }

        self.stream
            .lock()
            .await
            .rmdir(&self.full_path(path))
            .await?;
        Ok(())
    }
}

fn trim_root(path: &str) -> String {
    path.trim_start_matches('/').to_string()
}

fn parse_line(line: &str) -> ParseResult<suppaftp::list::File> {
    ListParser::parse_posix(line).or_else(|_| ListParser::parse_dos(line))
}

fn system_time_to_utc(time: SystemTime) -> Option<DateTime<Utc>> {
    let secs = time.duration_since(UNIX_EPOCH).ok()?;
    DateTime::from_timestamp(secs.as_secs() as i64, 0)
}