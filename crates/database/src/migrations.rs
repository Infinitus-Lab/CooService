//! 迁移：按文件名顺序执行 `migrations/*.sql`，已执行的记在 `_migrations` 表里。

use std::{fs, io, path::Path};

use crate::{Database, error::DatabaseError};

const MIGRATIONS_TABLE: &str = "_migrations";

pub async fn run(db: &Database, dir: &Path) -> Result<(), DatabaseError> {
    let mut conn = db.conn().await?;

    conn.batch_execute(&format!(
        "CREATE TABLE IF NOT EXISTS {MIGRATIONS_TABLE} (
            name        TEXT PRIMARY KEY,
            applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
        )"
    ))
    .await?;

    let applied: Vec<String> = conn
        .query(&format!("SELECT name FROM {MIGRATIONS_TABLE}"), &[])
        .await?
        .iter()
        .map(|row| row.get("name"))
        .collect();

    for path in pending_migrations(dir, &applied)? {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                DatabaseError::Migration(format!("invalid migration file name: {path:?}"))
            })?
            .to_string();

        let sql = fs::read_to_string(&path)
            .map_err(|e| DatabaseError::Migration(format!("read {}: {e}", path.display())))?;

        let tx = conn.transaction().await?;
        tx.batch_execute(&sql).await?;
        tx.execute(
            &format!("INSERT INTO {MIGRATIONS_TABLE} (name) VALUES ($1)"),
            &[&name],
        )
        .await?;
        tx.commit().await?;

        tracing::info!(migration = %name, "migration applied");
    }

    Ok(())
}

/// 目录下未执行过的迁移文件，按文件名排序。
fn pending_migrations(
    dir: &Path,
    applied: &[String],
) -> Result<Vec<std::path::PathBuf>, DatabaseError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            tracing::warn!(dir = %dir.display(), "migrations dir not found, skipping");
            return Ok(Vec::new());
        }
        Err(e) => {
            return Err(DatabaseError::Migration(format!(
                "read {}: {e}",
                dir.display()
            )));
        }
    };

    let mut files: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("sql"))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !applied.contains(&n.to_string()))
        })
        .collect();

    files.sort();
    Ok(files)
}
