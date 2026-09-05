//! `channel` / `channel_release` / `channel_diff` 的查询。
//!
//! sha256 在库里是 `bytea`，接口层一律用十六进制字符串，SQL 里 `decode($n, 'hex')` 转换。

use uuid::Uuid;

use crate::{Database, error::DatabaseError};

/// 通道一行。
pub struct ChannelRow {
    pub guid: Uuid,
    pub app_id: Uuid,
    pub tag_name: String,
    pub latest_version: String,
    pub raw_sha256: String,
    pub raw_size: i64,
    pub is_default: bool,
}

pub struct DiffRow {
    pub base_sha256: String,
    pub patch_sha256: String,
    pub algo: Option<String>,
    pub size: i64,
}

pub struct ReleaseRow {
    pub version: String,
    pub sha256: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

const COLUMNS: &str = "
    SELECT c.guid,
           c.app_id,
           c.tag_name,
           c.latest_version,
           encode(c.latest_sha256, 'hex'),
           r.size,
           c.is_default
    FROM channel c
    JOIN resource r ON r.sha256 = c.latest_sha256";

/// 客户端接口用：默认通道排最前，其余按创建顺序。
pub async fn list_by_app(db: &Database, app_id: Uuid) -> Result<Vec<ChannelRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            &format!("{COLUMNS} WHERE c.app_id = $1 ORDER BY c.is_default DESC, c.created_at"),
            &[&app_id],
        )
        .await?;

    Ok(rows.iter().map(ChannelRow::from_row).collect())
}

/// 管理接口用：跨 app 全量。
pub async fn list_all(db: &Database) -> Result<Vec<ChannelRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            &format!("{COLUMNS} ORDER BY c.app_id, c.is_default DESC, c.created_at"),
            &[],
        )
        .await?;

    Ok(rows.iter().map(ChannelRow::from_row).collect())
}

pub async fn get(db: &Database, guid: Uuid) -> Result<Option<ChannelRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(&format!("{COLUMNS} WHERE c.guid = $1"), &[&guid])
        .await?;

    Ok(rows.first().map(ChannelRow::from_row))
}

pub async fn insert(
    db: &Database,
    guid: Uuid,
    app_id: Uuid,
    tag_name: &str,
    latest_version: &str,
    latest_sha256: &str,
    is_default: bool,
) -> Result<(), DatabaseError> {
    let mut conn = db.conn().await?;
    let tx = conn.transaction().await?;

    // 设为默认时先清掉同 app 其他通道的默认标记
    if is_default {
        tx.execute(
            "UPDATE channel SET is_default = false WHERE app_id = $1",
            &[&app_id],
        )
        .await?;
    }

    tx.execute(
        "INSERT INTO channel (guid, app_id, tag_name, latest_version, latest_sha256, is_default)
         VALUES ($1, $2, $3, $4, decode($5, 'hex'), $6)",
        &[
            &guid,
            &app_id,
            &tag_name,
            &latest_version,
            &latest_sha256,
            &is_default,
        ],
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// 只改传了值的字段。设为默认时同 app 其他通道自动取消默认。
/// 顺序：先清别人再设目标——部分唯一索引 `(app_id) WHERE is_default` 是立即检查，
/// 反过来先设目标会先撞索引。返回是否命中。
pub async fn update(
    db: &Database,
    guid: Uuid,
    tag_name: Option<&str>,
    latest_version: Option<&str>,
    latest_sha256: Option<&str>,
    is_default: Option<bool>,
) -> Result<bool, DatabaseError> {
    let mut conn = db.conn().await?;
    let tx = conn.transaction().await?;

    if is_default == Some(true) {
        tx.execute(
            "UPDATE channel SET is_default = false
             WHERE app_id = (SELECT app_id FROM channel WHERE guid = $1)
               AND guid <> $1",
            &[&guid],
        )
        .await?;
    }

    let affected = tx
        .execute(
            "UPDATE channel SET
                 tag_name       = COALESCE($2, tag_name),
                 latest_version = COALESCE($3, latest_version),
                 latest_sha256  = COALESCE(decode($4, 'hex'), latest_sha256),
                 is_default     = COALESCE($5, is_default),
                 updated_at     = now()
             WHERE guid = $1",
            &[
                &guid,
                &tag_name,
                &latest_version,
                &latest_sha256,
                &is_default,
            ],
        )
        .await?;

    tx.commit().await?;
    Ok(affected > 0)
}

pub async fn delete(db: &Database, guid: Uuid) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute("DELETE FROM channel WHERE guid = $1", &[&guid])
        .await?;

    Ok(affected > 0)
}

pub async fn list_releases(db: &Database, guid: Uuid) -> Result<Vec<ReleaseRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT version, encode(sha256, 'hex'), created_at
             FROM channel_release
             WHERE channel_guid = $1
             ORDER BY created_at DESC",
            &[&guid],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| ReleaseRow {
            version: row.get(0),
            sha256: row.get(1),
            created_at: row.get(2),
        })
        .collect())
}

pub async fn insert_release(
    db: &Database,
    guid: Uuid,
    version: &str,
    sha256: &str,
) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO channel_release (channel_guid, version, sha256)
         VALUES ($1, $2, decode($3, 'hex'))
         ON CONFLICT (channel_guid, version) DO UPDATE SET sha256 = EXCLUDED.sha256",
        &[&guid, &version, &sha256],
    )
    .await?;
    Ok(())
}

pub async fn delete_release(
    db: &Database,
    guid: Uuid,
    version: &str,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "DELETE FROM channel_release WHERE channel_guid = $1 AND version = $2",
            &[&guid, &version],
        )
        .await?;

    Ok(affected > 0)
}

/// 该通道下所有差分：同通道跨一个版本 + 跨通道，都在一张表里。
pub async fn list_diffs(db: &Database, guid: Uuid) -> Result<Vec<DiffRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            "SELECT encode(base_sha256, 'hex'),
                    encode(patch_sha256, 'hex'),
                    algo,
                    size
             FROM channel_diff
             WHERE channel_guid = $1
             ORDER BY created_at",
            &[&guid],
        )
        .await?;

    Ok(rows.iter().map(DiffRow::from_row).collect())
}

pub async fn insert_diff(
    db: &Database,
    guid: Uuid,
    base_sha256: &str,
    patch_sha256: &str,
    algo: Option<&str>,
    size: i64,
) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO channel_diff (channel_guid, base_sha256, patch_sha256, algo, size)
         VALUES ($1, decode($2, 'hex'), decode($3, 'hex'), $4, $5)
         ON CONFLICT (channel_guid, base_sha256) DO UPDATE
             SET patch_sha256 = EXCLUDED.patch_sha256,
                 algo         = EXCLUDED.algo,
                 size         = EXCLUDED.size",
        &[&guid, &base_sha256, &patch_sha256, &algo, &size],
    )
    .await?;
    Ok(())
}

pub async fn delete_diff(
    db: &Database,
    guid: Uuid,
    base_sha256: &str,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "DELETE FROM channel_diff
             WHERE channel_guid = $1 AND base_sha256 = decode($2, 'hex')",
            &[&guid, &base_sha256],
        )
        .await?;

    Ok(affected > 0)
}

impl ChannelRow {
    fn from_row(row: &tokio_postgres::Row) -> Self {
        Self {
            guid: row.get(0),
            app_id: row.get(1),
            tag_name: row.get(2),
            latest_version: row.get(3),
            raw_sha256: row.get(4),
            raw_size: row.get(5),
            is_default: row.get(6),
        }
    }
}

impl DiffRow {
    fn from_row(row: &tokio_postgres::Row) -> Self {
        Self {
            base_sha256: row.get(0),
            patch_sha256: row.get(1),
            algo: row.get(2),
            size: row.get(3),
        }
    }
}
