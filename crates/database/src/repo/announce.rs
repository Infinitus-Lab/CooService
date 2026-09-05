//! `announce` / `app_announce` 的读写。
//!
//! 公告全局一份、与 app 解耦：app 通过 `app_announce` 引用（多 app 可共享同一条）。
//! 展示有效期 `[starts_at, expires_at)`：NULL 表示不限；客户端只看到被引用且当前可见的。

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Database, error::DatabaseError};

pub struct AnnounceRow {
    pub guid: Uuid,
    pub title: String,
    pub content: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 引用这条公告的 app 名（按名字排序，管理列表展示用）
    pub ref_apps: Vec<String>,
}

const SELECT: &str = "
    SELECT a.guid, a.title, a.content, a.starts_at, a.expires_at, a.created_at, a.updated_at,
           COALESCE(array_agg(app.name ORDER BY app.name)
                    FILTER (WHERE app.name IS NOT NULL), '{}')
    FROM announce a
    LEFT JOIN app_announce aa ON aa.announce_guid = a.guid
    LEFT JOIN app ON app.id = aa.app_id";

const GROUP: &str =
    "GROUP BY a.guid, a.title, a.content, a.starts_at, a.expires_at, a.created_at, a.updated_at";

fn from_row(row: &tokio_postgres::Row) -> AnnounceRow {
    AnnounceRow {
        guid: row.get(0),
        title: row.get(1),
        content: row.get(2),
        starts_at: row.get(3),
        expires_at: row.get(4),
        created_at: row.get(5),
        updated_at: row.get(6),
        ref_apps: row.get(7),
    }
}

/// 管理接口用：全量（含未开始 / 已过期），新的在前。
pub async fn list_all(db: &Database) -> Result<Vec<AnnounceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(&format!("{SELECT} {GROUP} ORDER BY a.created_at DESC"), &[])
        .await?;

    Ok(rows.iter().map(from_row).collect())
}

/// 管理接口用：被指定 app 引用的公告。
pub async fn list_by_app(db: &Database, app_id: Uuid) -> Result<Vec<AnnounceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            &format!(
                "{SELECT} WHERE EXISTS (
                     SELECT 1 FROM app_announce x
                     WHERE x.announce_guid = a.guid AND x.app_id = $1)
                 {GROUP} ORDER BY a.created_at DESC"
            ),
            &[&app_id],
        )
        .await?;

    Ok(rows.iter().map(from_row).collect())
}

/// 客户端接口用：被引用且**当前可见**（起点已到、终点未过，NULL 边不限）。
pub async fn list_visible_by_app(
    db: &Database,
    app_id: Uuid,
) -> Result<Vec<AnnounceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(
            &format!(
                "{SELECT} WHERE EXISTS (
                     SELECT 1 FROM app_announce x
                     WHERE x.announce_guid = a.guid AND x.app_id = $1)
                 AND (a.starts_at IS NULL OR a.starts_at <= now())
                 AND (a.expires_at IS NULL OR a.expires_at > now())
                 {GROUP} ORDER BY a.created_at DESC"
            ),
            &[&app_id],
        )
        .await?;

    Ok(rows.iter().map(from_row).collect())
}

pub async fn get(db: &Database, guid: Uuid) -> Result<Option<AnnounceRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query(&format!("{SELECT} WHERE a.guid = $1 {GROUP}"), &[&guid])
        .await?;

    Ok(rows.first().map(from_row))
}

pub async fn insert(
    db: &Database,
    guid: Uuid,
    title: &str,
    content: &str,
    starts_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO announce (guid, title, content, starts_at, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
        &[&guid, &title, &content, &starts_at, &expires_at],
    )
    .await?;
    Ok(())
}

/// 全量覆盖（标题 / 内容 / 有效期）：公告很小且编辑表单总是整单提交，
/// 全量语义下"把有效期清成 NULL"不会和"不改"混淆。`updated_at` 自动刷新。
pub async fn update(
    db: &Database,
    guid: Uuid,
    title: &str,
    content: &str,
    starts_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "UPDATE announce SET
                 title      = $2,
                 content    = $3,
                 starts_at  = $4,
                 expires_at = $5,
                 updated_at = now()
             WHERE guid = $1",
            &[&guid, &title, &content, &starts_at, &expires_at],
        )
        .await?;

    Ok(affected > 0)
}

pub async fn delete(db: &Database, guid: Uuid) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute("DELETE FROM announce WHERE guid = $1", &[&guid])
        .await?;

    Ok(affected > 0)
}

// ---------- app 引用 ----------

pub async fn link(db: &Database, app_id: Uuid, announce_guid: Uuid) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute(
        "INSERT INTO app_announce (app_id, announce_guid) VALUES ($1, $2)
         ON CONFLICT (app_id, announce_guid) DO NOTHING",
        &[&app_id, &announce_guid],
    )
    .await?;
    Ok(())
}

pub async fn unlink(
    db: &Database,
    app_id: Uuid,
    announce_guid: Uuid,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "DELETE FROM app_announce WHERE app_id = $1 AND announce_guid = $2",
            &[&app_id, &announce_guid],
        )
        .await?;

    Ok(affected > 0)
}
