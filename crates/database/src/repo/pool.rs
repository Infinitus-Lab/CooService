//! `resource_pool` 表的读写。
//!
//! 只存配置（终结点 / 密钥 / 公开终结点 / kind 相关参数），
//! 真正的连接由 `resource` crate 拿着这些配置去建。

use serde_json::Value;

use crate::{Database, error::DatabaseError};

pub struct PoolRow {
    pub id: String,
    pub kind: String,
    pub endpoint: String,
    pub public_endpoint: String,
    pub secret: String,
    pub config: Value,
    /// 是否参与公开下载 302 重定向
    pub is_public: bool,
}

impl PoolRow {
    fn from_row(row: &tokio_postgres::Row) -> Self {
        Self {
            id: row.get(0),
            kind: row.get(1),
            endpoint: row.get(2),
            public_endpoint: row.get(3),
            secret: row.get(4),
            config: row.get(5),
            is_public: row.get(6),
        }
    }
}

const COLUMNS: &str =
    "SELECT id, kind, endpoint, public_endpoint, secret, config, is_public FROM resource_pool";

pub async fn list(db: &Database) -> Result<Vec<PoolRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn.query(&format!("{COLUMNS} ORDER BY id"), &[]).await?;
    Ok(rows.iter().map(PoolRow::from_row).collect())
}

pub async fn get(db: &Database, id: &str) -> Result<Option<PoolRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(&format!("{COLUMNS} WHERE id = $1"), &[&id])
        .await?;

    Ok(rows.first().map(PoolRow::from_row))
}

/// 新建资源池的完整配置。
pub struct NewPool<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub endpoint: &'a str,
    pub public_endpoint: &'a str,
    pub secret: &'a str,
    pub config: &'a Value,
    pub is_public: bool,
}

pub async fn insert(db: &Database, pool: &NewPool<'_>) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO resource_pool (id, kind, endpoint, public_endpoint, secret, config, is_public)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        &[
            &pool.id,
            &pool.kind,
            &pool.endpoint,
            &pool.public_endpoint,
            &pool.secret,
            pool.config,
            &pool.is_public,
        ],
    )
    .await?;
    Ok(())
}

/// 部分更新：字段为 `None` 表示不动。返回是否命中这一行。
pub struct PoolUpdate<'a> {
    pub id: &'a str,
    pub endpoint: Option<&'a str>,
    pub public_endpoint: Option<&'a str>,
    pub secret: Option<&'a str>,
    pub config: Option<&'a Value>,
    pub is_public: Option<bool>,
}

pub async fn update(db: &Database, pool: &PoolUpdate<'_>) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "UPDATE resource_pool SET
                 endpoint        = COALESCE($2, endpoint),
                 public_endpoint = COALESCE($3, public_endpoint),
                 secret          = COALESCE($4, secret),
                 config          = COALESCE($5, config),
                 is_public       = COALESCE($6, is_public)
             WHERE id = $1",
            &[
                &pool.id,
                &pool.endpoint,
                &pool.public_endpoint,
                &pool.secret,
                &pool.config,
                &pool.is_public,
            ],
        )
        .await?;

    Ok(affected > 0)
}

pub async fn delete(db: &Database, id: &str) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute("DELETE FROM resource_pool WHERE id = $1", &[&id])
        .await?;

    Ok(affected > 0)
}

/// 每个池的登记量统计：资源数 / 总量 / 最近一次写入（location 新增即写入）。
pub struct PoolStats {
    pub object_count: i64,
    pub total_size: i64,
    pub last_write_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn stats(
    db: &Database,
) -> Result<std::collections::HashMap<String, PoolStats>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT rl.pool_id, COUNT(*), COALESCE(SUM(r.size), 0)::BIGINT, MAX(rl.created_at)
             FROM resource_location rl
             JOIN resource r ON r.sha256 = rl.sha256
             GROUP BY rl.pool_id",
            &[],
        )
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: String = row.get(0);
            (
                id.clone(),
                PoolStats {
                    object_count: row.get(1),
                    total_size: row.get(2),
                    last_write_at: row.get(3),
                },
            )
        })
        .collect())
}
