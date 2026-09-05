//! 资源池的错误类型。

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("s3 error: {0}")]
    S3(#[from] s3::error::S3Error),

    #[error("ftp error: {0}")]
    Ftp(#[from] suppaftp::FtpError),

    #[error("ftp list parse error: {0}")]
    ListParse(#[from] suppaftp::list::ParseError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid config: {0}")]
    Config(String),

    #[error("pool not found: {0}")]
    NotFound(String),
}
