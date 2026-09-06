//! `/api/v1/admin/announces`：公告的增删改查。
//!
//! 展示有效期 `[starts_at, expires_at)`，NULL 表示不限；
//! 管理台账看全部（含未开始 / 已过期），客户端只看到当前可见的（见 `routes/app.rs`）。
//! 本文件只放 admin 接口——整棵子树在 `/api/v1/admin` 下，由 `admin/mod.rs` 的
//! `X-Admin-Key` 中间件保护；客户端读取在 `routes/app.rs`，不要混写。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
};
use chrono::{DateTime, Utc};
use database::repo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ApiOk, ApiResult, AppError, response::with_status, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{guid}", get(detail).patch(modify).delete(remove))
}

#[derive(Debug, Serialize)]
pub struct AnnounceView {
    pub guid: Uuid,
    pub title: String,
    pub content: String,
    /// 展示起点，NULL 不限
    pub starts_at: Option<DateTime<Utc>>,
    /// 展示终点，NULL 不限
    pub expires_at: Option<DateTime<Utc>>,
    /// 当前可见性：permanent / scheduled / active / expired
    pub status: &'static str,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 引用这条公告的 app 名
    pub ref_apps: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnounceRequest {
    pub title: String,
    pub content: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// PATCH 全量语义：四个字段一次给全，NULL 表示清掉有效期。
/// 公告很小、编辑表单总是整单提交，全量能让"清成不限"和"不改"不混淆。
#[derive(Debug, Deserialize)]
pub struct UpdateAnnounceRequest {
    pub title: String,
    pub content: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// 按 app 过滤
    pub app_id: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(filter): Query<ListParams>,
) -> ApiResult<Vec<AnnounceView>> {
    // 按 app 过滤直接按 id 查引用关系（app.name 无唯一约束，按名字过滤会串台）
    let rows = match &filter.app_id {
        Some(raw) => {
            let id = raw
                .parse::<Uuid>()
                .map_err(|e| AppError::BadRequest(format!("invalid app_id: {e}")))?;
            repo::announce::list_by_app(&state.db, id).await?
        }
        None => repo::announce::list_all(&state.db).await?,
    };

    Ok(ApiOk(rows.into_iter().map(AnnounceView::from_row).collect()))
}

async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateAnnounceRequest>,
) -> Result<Response, AppError> {
    validate_period(body.starts_at, body.expires_at)?;

    let guid = Uuid::new_v4();
    repo::announce::insert(
        &state.db,
        guid,
        &body.title,
        &body.content,
        body.starts_at,
        body.expires_at,
    )
    .await?;

    let row = repo::announce::get(&state.db, guid)
        .await?
        .ok_or_else(|| AppError::NotFound(guid.to_string()))?;

    Ok(with_status(
        StatusCode::CREATED,
        AnnounceView::from_row(row),
    ))
}

async fn detail(Path(guid): Path<Uuid>, State(state): State<AppState>) -> ApiResult<AnnounceView> {
    let row = repo::announce::get(&state.db, guid)
        .await?
        .ok_or_else(|| AppError::NotFound(guid.to_string()))?;

    Ok(ApiOk(AnnounceView::from_row(row)))
}

async fn modify(
    Path(guid): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<UpdateAnnounceRequest>,
) -> ApiResult<AnnounceView> {
    validate_period(body.starts_at, body.expires_at)?;

    if !repo::announce::update(
        &state.db,
        guid,
        &body.title,
        &body.content,
        body.starts_at,
        body.expires_at,
    )
    .await?
    {
        return Err(AppError::NotFound(guid.to_string()));
    }

    let row = repo::announce::get(&state.db, guid)
        .await?
        .ok_or_else(|| AppError::NotFound(guid.to_string()))?;

    Ok(ApiOk(AnnounceView::from_row(row)))
}

async fn remove(Path(guid): Path<Uuid>, State(state): State<AppState>) -> ApiResult<Uuid> {
    if !repo::announce::delete(&state.db, guid).await? {
        return Err(AppError::NotFound(guid.to_string()));
    }

    Ok(ApiOk(guid))
}

/// 两端都填时必须 start < end（DB CHECK 同规则，这里提前给 400）。
fn validate_period(
    starts_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    if let (Some(starts_at), Some(expires_at)) = (starts_at, expires_at)
        && expires_at <= starts_at
    {
        return Err(AppError::BadRequest(
            "expires_at must be after starts_at".into(),
        ));
    }
    Ok(())
}

impl AnnounceView {
    pub fn from_row(row: repo::announce::AnnounceRow) -> Self {
        let status = visibility(&row);
        Self {
            guid: row.guid,
            title: row.title,
            content: row.content,
            starts_at: row.starts_at,
            expires_at: row.expires_at,
            status,
            created_at: row.created_at,
            updated_at: row.updated_at,
            ref_apps: row.ref_apps,
        }
    }
}

/// 当前可见性：permanent（不限）/ scheduled（未开始）/ active（展示中）/ expired（已过期）。
fn visibility(row: &repo::announce::AnnounceRow) -> &'static str {
    let now = Utc::now();
    match (row.starts_at, row.expires_at) {
        (None, None) => "permanent",
        (Some(starts), None) => {
            if now < starts {
                "scheduled"
            } else {
                "active"
            }
        }
        (None, Some(expires)) => {
            if now >= expires {
                "expired"
            } else {
                "active"
            }
        }
        (Some(starts), Some(expires)) => {
            if now < starts {
                "scheduled"
            } else if now >= expires {
                "expired"
            } else {
                "active"
            }
        }
    }
}
