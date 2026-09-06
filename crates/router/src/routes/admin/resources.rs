//! `/api/v1/admin/resources`：资源的管理接口。
//!
//! 领域边界：资源管理只负责**本地库**——上传（永远写本地，可带显示名）、列表、详情、删除。
//! 资源进入哪个资源池、池内同步，都是资源池管理的事（见 `pools.rs` 的池成员接口），
//! 本文件不碰池。本文件只放 admin 接口——整棵子树在 `/api/v1/admin` 下，
//! 由 `admin/mod.rs` 的 `X-Admin-Key` 中间件保护；公开下载在 `routes/resources.rs`。

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
};
use chrono::{DateTime, Utc};
use database::{DatabaseError, repo};
use futures_util::TryStreamExt;
use resource::{key::normalize_sha256, key::object_key, pool::RemotePool};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;
use uuid::Uuid;

use crate::{ApiOk, ApiResult, AppError, auth::hex_encode, response::with_status, state::AppState};

/// `RequestBodyLimitLayer` 读流中途触发上限时，错误链里会挂着 `LengthLimitError`。
fn is_length_limit_error(e: &std::io::Error) -> bool {
    use std::error::Error as _;

    let mut source = e.source();
    while let Some(err) = source {
        if err.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        source = err.source();
    }
    false
}

pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/{sha256}", get(detail).patch(rename).delete(remove))
}

#[derive(Debug, Serialize)]
pub struct ResourceView {
    pub sha256: String,
    pub size: i64,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    /// 这份内容存在的池 id
    pub pools: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ResourceDetailView {
    pub sha256: String,
    pub size: i64,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub pools: Vec<String>,
    /// 引用它的 app 名
    pub ref_apps: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// 只列存在于该池的
    pub pool: Option<String>,
    /// 精确匹配一个 sha256
    pub sha256: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(filter): Query<ListParams>,
) -> ApiResult<Vec<ResourceView>> {
    // 过滤用的 sha256 也归一，避免大写输入查不到（非法值视为无过滤）
    let filter_sha = filter
        .sha256
        .as_deref()
        .and_then(|s| normalize_sha256(s).ok());
    let rows = repo::resource::list_all(&state.db).await?;

    Ok(ApiOk(
        rows.into_iter()
            .filter(|row| {
                filter
                    .pool
                    .as_ref()
                    .is_none_or(|p| row.pools.iter().any(|x| x == p))
                    && filter_sha.as_ref().is_none_or(|s| &row.sha256 == s)
            })
            .map(|row| ResourceView {
                sha256: row.sha256,
                size: row.size,
                name: row.name,
                created_at: row.created_at,
                pools: row.pools,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct UploadParams {
    /// 可选显示名，仅管理端识别用
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadedView {
    pub sha256: String,
    pub size: u64,
    pub name: Option<String>,
}

/// 上传：临时文件（边写边算 sha256）→ 本地正式副本 → 入库。
/// 只落本地；进池是资源池管理的事（`pools.rs`）。
/// 路由装配在顶层（`crate::router`），挂请求体上限 + admin 鉴权，且豁免全局超时。
pub async fn upload(
    State(state): State<AppState>,
    Query(params): Query<UploadParams>,
    body: Body,
) -> Result<Response, AppError> {
    let name = params
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let tmp_dir = state.local.root().join("tmp");
    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create temp dir: {e}")))?;
    let tmp_path = tmp_dir.join(Uuid::new_v4().to_string());

    // 1. body → 临时文件，边写边算 sha256
    let stream = body.into_data_stream().map_err(std::io::Error::other);
    let mut reader = HashingReader::new(StreamReader::new(stream));
    let size = {
        let mut file = match tokio::fs::File::create(&tmp_path).await {
            Ok(file) => file,
            Err(e) => return Err(AppError::Internal(anyhow::anyhow!("create temp file: {e}"))),
        };
        if let Err(e) = tokio::io::copy(&mut reader, &mut file).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            // 超限（RequestBodyLimitLayer 的 LengthLimitError）映射 413，其余读失败才是 400
            if is_length_limit_error(&e) {
                return Err(AppError::PayloadTooLarge);
            }
            return Err(AppError::BadRequest(format!(
                "upload body read failed: {e}"
            )));
        }
        file.metadata()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("read temp file size: {e}")))?
            .len()
    };
    let sha256 = reader.hex();
    let key = object_key(&sha256).expect("hash computed here is valid hex");

    let existed = match repo::resource::exists(&state.db, &sha256).await {
        Ok(existed) => existed,
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(e.into());
        }
    };

    // 2. 本地正式副本（与 tmp 同文件系统，rename 原子，先落真身后再入库）
    let final_path = state.local.root().join(&key);
    if let Some(parent) = final_path.parent()
        && let Err(e) = tokio::fs::create_dir_all(parent).await
    {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(AppError::Internal(anyhow::anyhow!("create local dir: {e}")));
    }
    if let Err(e) = tokio::fs::rename(&tmp_path, &final_path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(AppError::Internal(anyhow::anyhow!(
            "local copy rename failed: {e}"
        )));
    }

    // 3. 入库（内容寻址幂等；name 只在新建时写入）
    let size_i64 =
        i64::try_from(size).map_err(|_| AppError::Internal(anyhow::anyhow!("file too large")))?;
    if let Err(e) = repo::resource::insert(&state.db, &sha256, size_i64, name).await {
        let _ = tokio::fs::remove_file(&final_path).await;
        return Err(e.into());
    }

    let status = if existed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok(with_status(
        status,
        UploadedView {
            sha256,
            size,
            name: name.map(str::to_string),
        },
    ))
}

async fn detail(
    Path(sha256): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<ResourceDetailView> {
    let sha256 = normalize_sha256(&sha256).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let row = repo::resource::detail(&state.db, &sha256)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("resource {sha256}")))?;

    Ok(ApiOk(ResourceDetailView {
        sha256: row.sha256,
        size: row.size,
        name: row.name,
        created_at: row.created_at,
        pools: row.pools,
        ref_apps: row.ref_apps,
    }))
}

async fn remove(Path(sha256): Path<String>, State(state): State<AppState>) -> ApiResult<String> {
    let sha256 = normalize_sha256(&sha256).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let key = object_key(&sha256).expect("normalized above");

    let row = repo::resource::get(&state.db, &sha256)
        .await?
        .ok_or_else(|| AppError::NotFound(sha256.clone()))?;
    let pools = row.pools;

    // 先删库：被 app / channel 引用时 FK（23503）挡住，物理副本一个都不动
    match repo::resource::delete(&state.db, &sha256).await {
        Ok(true) => {}
        Ok(false) => return Err(AppError::NotFound(sha256.clone())),
        Err(DatabaseError::Constraint { .. }) => {
            return Err(AppError::Conflict(format!(
                "resource {sha256} is referenced by an app or channel"
            )));
        }
        Err(e) => return Err(e.into()),
    }

    // 库删成功后再清物理副本，尽力而为；删不掉的留孤儿，下次上传同内容会覆盖
    for pool_id in &pools {
        if let Some(pool) = state.resources.get(pool_id)
            && let Err(e) = pool.delete_file(&key).await
        {
            tracing::error!(pool = %pool_id, sha256, error = %e, "delete pool object failed, orphan left");
        }
    }
    if let Err(e) = state.local.delete_file(&key).await {
        tracing::error!(sha256, error = %e, "delete local copy failed, orphan left");
    }
    state.sync.notify();

    Ok(ApiOk(sha256))
}

/// 修改资源显示名（资源自己的名字，全局）；空串或 null 表示清空。
#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub name: Option<String>,
}

async fn rename(
    Path(sha256): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> ApiResult<ResourceDetailView> {
    let sha256 = normalize_sha256(&sha256).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if !repo::resource::rename(&state.db, &sha256, name).await? {
        return Err(AppError::NotFound(format!("resource {sha256}")));
    }

    let row = repo::resource::detail(&state.db, &sha256)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("resource {sha256}")))?;
    Ok(ApiOk(ResourceDetailView {
        sha256: row.sha256,
        size: row.size,
        name: row.name,
        created_at: row.created_at,
        pools: row.pools,
        ref_apps: row.ref_apps,
    }))
}

/// 边读边算 sha256 的只读包装：一次遍历拿到内容 + 哈希，不落第二个缓冲区。
struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
}

impl<R> HashingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    fn hex(&self) -> String {
        hex_encode(&self.hasher.clone().finalize())
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for HashingReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        self.hasher.update(&buf.filled()[before..]);
        result
    }
}
