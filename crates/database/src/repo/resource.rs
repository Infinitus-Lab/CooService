//! `resource` / `resource_location` / `app_resource` 三张表的读写。
//!
//! 约定：sha256 在库里存 `bytea`，本层一律用 hex 字符串进出，SQL 里 `decode($n, 'hex')` 转换。

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Database, error::DatabaseError};

pub struct ResourceRow {
    pub sha256: String,
    pub size: i64,
    /// 可选显示名（上传时指定）
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    /// 这份内容存在的池 id（`resource_location` 聚合）
    pub pools: Vec<String>,
}

impl ResourceRow {
    fn from_row(row: &tokio_postgres::Row) -> Self {
        Self {
            sha256: row.get(0),
            size: row.get(1),
            name: row.get(2),
            created_at: row.get(3),
            pools: row.get(4),
        }
    }
}

const SELECT: &str = "SELECT encode(r.sha256, 'hex'), r.size, r.name, r.created_at,
             COALESCE(array_agg(rl.pool_id ORDER BY rl.pool_id)
                      FILTER (WHERE rl.pool_id IS NOT NULL), '{}')
      FROM resource r
      LEFT JOIN resource_location rl ON rl.sha256 = r.sha256";

/// 全量列表，按资源创建时间倒序（最新的在前）。
pub async fn list_all(db: &Database) -> Result<Vec<ResourceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            &format!("{SELECT} GROUP BY r.sha256, r.size, r.name, r.created_at ORDER BY r.created_at DESC"),
            &[],
        )
        .await?;

    Ok(rows.iter().map(ResourceRow::from_row).collect())
}

pub async fn get(db: &Database, sha256: &str) -> Result<Option<ResourceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            &format!(
                "{SELECT} WHERE r.sha256 = decode($1, 'hex')
                      GROUP BY r.sha256, r.size, r.name, r.created_at"
            ),
            &[&sha256],
        )
        .await?;

    Ok(rows.first().map(ResourceRow::from_row))
}

/// 详情：资源信息 + 所在池 + 引用它的 app 名（详情弹窗用）。
pub struct ResourceDetail {
    pub sha256: String,
    pub size: i64,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub pools: Vec<String>,
    pub ref_apps: Vec<String>,
}

pub async fn detail(db: &Database, sha256: &str) -> Result<Option<ResourceDetail>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT encode(r.sha256, 'hex'), r.size, r.name, r.created_at,
                    COALESCE(array_agg(DISTINCT rl.pool_id ORDER BY rl.pool_id)
                             FILTER (WHERE rl.pool_id IS NOT NULL), '{}'),
                    COALESCE(array_agg(DISTINCT app.name ORDER BY app.name)
                             FILTER (WHERE app.name IS NOT NULL), '{}')
             FROM resource r
             LEFT JOIN resource_location rl ON rl.sha256 = r.sha256
             LEFT JOIN app_resource ar ON ar.sha256 = r.sha256
             LEFT JOIN app ON app.id = ar.app_id
             WHERE r.sha256 = decode($1, 'hex')
             GROUP BY r.sha256, r.size, r.name, r.created_at",
            &[&sha256],
        )
        .await?;

    Ok(rows.first().map(|row| ResourceDetail {
        sha256: row.get(0),
        size: row.get(1),
        name: row.get(2),
        created_at: row.get(3),
        pools: row.get(4),
        ref_apps: row.get(5),
    }))
}

pub async fn exists(db: &Database, sha256: &str) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT 1 FROM resource WHERE sha256 = decode($1, 'hex')",
            &[&sha256],
        )
        .await?;

    Ok(!rows.is_empty())
}

/// 登记资源本身；已存在时静默忽略（内容寻址天然幂等），name 仅在新建时生效。
pub async fn insert(
    db: &Database,
    sha256: &str,
    size: i64,
    name: Option<&str>,
) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO resource (sha256, size, name) VALUES (decode($1, 'hex'), $2, $3)
         ON CONFLICT (sha256) DO NOTHING",
        &[&sha256, &size, &name],
    )
    .await?;
    Ok(())
}

/// 一个池里登记的对象（池成员管理）。
pub struct PoolResourceRow {
    pub sha256: String,
    pub name: Option<String>,
    pub size: i64,
    pub added_at: DateTime<Utc>,
}

pub async fn list_by_pool(
    db: &Database,
    pool_id: &str,
) -> Result<Vec<PoolResourceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT encode(rl.sha256, 'hex'), r.name, r.size, rl.created_at
             FROM resource_location rl
             JOIN resource r ON r.sha256 = rl.sha256
             WHERE rl.pool_id = $1
             ORDER BY rl.created_at DESC",
            &[&pool_id],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| PoolResourceRow {
            sha256: row.get(0),
            name: row.get(1),
            size: row.get(2),
            added_at: row.get(3),
        })
        .collect())
}

/// 删除资源及其所有池副本记录。行被 app_resource / channel 引用时由 FK 挡住，返回错误。
pub async fn delete(db: &Database, sha256: &str) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "DELETE FROM resource WHERE sha256 = decode($1, 'hex')",
            &[&sha256],
        )
        .await?;

    Ok(affected > 0)
}

pub async fn add_location(db: &Database, sha256: &str, pool_id: &str) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO resource_location (sha256, pool_id)
         VALUES (decode($1, 'hex'), $2)
         ON CONFLICT (sha256, pool_id) DO NOTHING",
        &[&sha256, &pool_id],
    )
    .await?;
    Ok(())
}

pub async fn remove_location(
    db: &Database,
    sha256: &str,
    pool_id: &str,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "DELETE FROM resource_location WHERE sha256 = decode($1, 'hex') AND pool_id = $2",
            &[&sha256, &pool_id],
        )
        .await?;

    Ok(affected > 0)
}

/// 全量 location 按池分组的 sha256 集合（同步引擎的"期望登记"清单）。
pub async fn shas_by_pool(
    db: &Database,
) -> Result<std::collections::HashMap<String, std::collections::HashSet<String>>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT pool_id, encode(sha256, 'hex') FROM resource_location",
            &[],
        )
        .await?;

    let mut map: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for row in rows {
        let pool_id: String = row.get(0);
        let sha256: String = row.get(1);
        map.entry(pool_id).or_default().insert(sha256);
    }
    Ok(map)
}

/// 修改资源显示名；传 `None` 清空。返回是否命中。
pub async fn rename(
    db: &Database,
    sha256: &str,
    name: Option<&str>,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "UPDATE resource SET name = $2 WHERE sha256 = decode($1, 'hex')",
            &[&sha256, &name],
        )
        .await?;

    Ok(affected > 0)
}

/// app 关联的资源（`app_resource`）。
pub struct AppResourceRow {
    pub sha256: String,
    pub size: i64,
}

/// app 维度引用资源，name 是可选别名（NULL 不参与唯一）。
pub async fn list_app_resources(
    db: &Database,
    app_id: Uuid,
) -> Result<Vec<AppResourceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT encode(ar.sha256, 'hex'), r.size
             FROM app_resource ar
             JOIN resource r ON r.sha256 = ar.sha256
             WHERE ar.app_id = $1
             ORDER BY ar.created_at",
            &[&app_id],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| AppResourceRow {
            sha256: row.get(0),
            size: row.get(1),
        })
        .collect())
}

/// 关联资源到 app（纯引用，不单独命名）。资源不存在时由 FK 挡住。
pub async fn link_app_resource(
    db: &Database,
    app_id: Uuid,
    sha256: &str,
) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO app_resource (app_id, sha256) VALUES ($1, decode($2, 'hex'))
         ON CONFLICT (app_id, sha256) DO NOTHING",
        &[&app_id, &sha256],
    )
    .await?;
    Ok(())
}

pub async fn unlink_app_resource(
    db: &Database,
    app_id: Uuid,
    sha256: &str,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "DELETE FROM app_resource WHERE app_id = $1 AND sha256 = decode($2, 'hex')",
            &[&app_id, &sha256],
        )
        .await?;

    Ok(affected > 0)
}
