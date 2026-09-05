//! `app` 表的读写。

use uuid::Uuid;

use crate::{Database, error::DatabaseError};

pub struct AppRow {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

impl AppRow {
    fn from_row(row: &tokio_postgres::Row) -> Self {
        Self {
            id: row.get(0),
            name: row.get(1),
            enabled: row.get(2),
        }
    }
}

/// 只取启用的，启动时载入内存用这个。
pub async fn list_enabled(db: &Database) -> Result<Vec<AppRow>, DatabaseError> {
    query_rows(db, "SELECT id, name, enabled FROM app WHERE enabled").await
}

/// 管理接口用：含禁用的，按创建顺序。
pub async fn list_all(db: &Database) -> Result<Vec<AppRow>, DatabaseError> {
    query_rows(db, "SELECT id, name, enabled FROM app ORDER BY created_at").await
}

/// 只改传了值的字段。返回是否命中这一行。
pub async fn update(
    db: &Database,
    id: Uuid,
    name: Option<&str>,
    enabled: Option<bool>,
) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "UPDATE app SET
                 name       = COALESCE($2, name),
                 enabled    = COALESCE($3, enabled),
                 updated_at = now()
             WHERE id = $1",
            &[&id, &name, &enabled],
        )
        .await?;

    Ok(affected > 0)
}

pub async fn get(db: &Database, id: Uuid) -> Result<Option<AppRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query("SELECT id, name, enabled FROM app WHERE id = $1", &[&id])
        .await?;

    Ok(rows.first().map(AppRow::from_row))
}

pub async fn exists(db: &Database, id: Uuid) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn
        .query("SELECT 1 FROM app WHERE id = $1", &[&id])
        .await?;

    Ok(!rows.is_empty())
}

pub async fn insert(db: &Database, id: Uuid, name: &str) -> Result<(), DatabaseError> {
    let conn = db.conn().await?;
    conn.execute("INSERT INTO app (id, name) VALUES ($1, $2)", &[&id, &name])
        .await?;
    Ok(())
}

/// 返回是否真的改到一行（id 不存在时返回 false）。
pub async fn set_enabled(db: &Database, id: Uuid, enabled: bool) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute(
            "UPDATE app SET enabled = $2, updated_at = now() WHERE id = $1",
            &[&id, &enabled],
        )
        .await?;

    Ok(affected > 0)
}

pub async fn delete(db: &Database, id: Uuid) -> Result<bool, DatabaseError> {
    let conn = db.conn().await?;
    let affected = conn
        .execute("DELETE FROM app WHERE id = $1", &[&id])
        .await?;

    Ok(affected > 0)
}

async fn query_rows(db: &Database, sql: &str) -> Result<Vec<AppRow>, DatabaseError> {
    let conn = db.conn().await?;
    let rows = conn.query(sql, &[]).await?;
    Ok(rows.iter().map(AppRow::from_row).collect())
}
