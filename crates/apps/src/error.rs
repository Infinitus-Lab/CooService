//! 应用管理的错误类型。

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] database::DatabaseError),

    #[error("app {0} not found")]
    NotFound(String),

    #[error("app {0} already exists")]
    AlreadyExists(String),
}
