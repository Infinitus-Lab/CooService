//! 连接池。

use deadpool_postgres::{Manager, Pool, Runtime};
use tokio_postgres::NoTls;

use crate::{config::DatabaseConfig, error::DatabaseError, migrations};

/// PostgreSQL 连接池。`Pool` 内部是 `Arc`，clone 代价很低。
#[derive(Clone)]
pub struct Database(Pool);

impl Database {
    /// 建池 -> 验证连通 -> 执行迁移，任一步失败即返回 Err（启动即失败）。
    pub async fn connect(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        let mut pg_config: tokio_postgres::Config = config
            .url
            .parse()
            .map_err(|e| DatabaseError::Connect(format!("invalid DATABASE_URL: {e}")))?;
        pg_config.connect_timeout(config.connect_timeout);

        let manager = Manager::new(pg_config, NoTls);
        let pool = Pool::builder(manager)
            .max_size(config.max_connections as usize)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| DatabaseError::Connect(format!("build pool: {e}")))?;

        let db = Self(pool);
        db.ping().await?;
        migrations::run(&db, &config.migrations_dir).await?;

        tracing::info!(
            max_connections = config.max_connections,
            "database connected"
        );
        Ok(db)
    }

    /// 取一条连接；池满时等待，直到 `connect_timeout` 超时。
    pub async fn conn(&self) -> Result<deadpool_postgres::Object, DatabaseError> {
        self.0.get().await.map_err(DatabaseError::Pool)
    }

    pub async fn ping(&self) -> Result<(), DatabaseError> {
        self.conn().await?.simple_query("SELECT 1").await?;
        Ok(())
    }
}
