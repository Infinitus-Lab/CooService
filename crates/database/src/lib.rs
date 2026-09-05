//! PostgreSQL 访问层：`Database` 持有连接池，启动时建连并执行迁移。
//!
//! 驱动 `tokio-postgres` 是异步的，直接用它，不要用同步的 `postgres` crate：
//! 后者内部自带 runtime 并 block_on，连接对象在 tokio runtime 里 drop 就会 panic。

pub mod config;
pub mod error;
pub mod migrations;
pub mod pool;
pub mod repo;

pub use config::DatabaseConfig;
pub use error::DatabaseError;
pub use pool::Database;
