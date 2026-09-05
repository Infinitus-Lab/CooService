//! 数据库错误类型。

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Connect(String),

    /// 约束违反（如外键 RESTRICT 挡住删除），`code` 是 SQLSTATE。
    #[error("database constraint ({code}): {message}")]
    Constraint { code: String, message: String },

    #[error("connection pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("migration failed: {0}")]
    Migration(String),
}

impl From<tokio_postgres::Error> for DatabaseError {
    fn from(e: tokio_postgres::Error) -> Self {
        // tokio_postgres 的 Display 只给 "db error"，真正的原因在 DbError 里
        match e.as_db_error() {
            Some(db) => match db.code().code() {
                // 23503: 外键违反；23505: 唯一约束违反（如重复 tag_name）
                "23503" | "23505" => Self::Constraint {
                    code: db.code().code().to_string(),
                    message: db.message().to_string(),
                },
                _ => Self::Connect(db.to_string()),
            },
            None => Self::Connect(e.to_string()),
        }
    }
}
