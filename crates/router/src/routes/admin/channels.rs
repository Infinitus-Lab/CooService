//! `/api/v1/admin/channels`：发版通道、版本记录、差分的增删改查。
//!
//! 建通道时 `latest_sha256` 必须指向已存在的资源（外键 RESTRICT），
//! 所以流程是：先有资源，再建/改通道。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::{delete, get},
};
use database::repo;
use resource::key::object_key;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ApiOk, ApiResult, AppError, response::with_status, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_all).post(create))
        .route("/{guid}", get(detail).patch(modify).delete(remove))
        .route("/{guid}/releases", get(list_releases).post(create_release))
        .route("/{guid}/releases/{version}", delete(delete_release))
        .route("/{guid}/diffs", get(list_diffs).post(create_diff))
        .route("/{guid}/diffs/{base_sha256}", delete(delete_diff))
}

#[derive(Debug, Serialize)]
pub struct ChannelView {
    pub guid: Uuid,
    pub app_id: Uuid,
    pub tag_name: String,
    pub latest_version: String,
    pub raw_sha256: String,
    pub raw_size: u64,
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub app_id: Uuid,
    pub tag_name: String,
    pub latest_version: String,
    /// 完整包资源的 sha256（十六进制），必须已存在于 `resource` 表
    pub latest_sha256: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub tag_name: Option<String>,
    pub latest_version: Option<String>,
    pub latest_sha256: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReleaseRequest {
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDiffRequest {
    pub base_sha256: String,
    pub patch_sha256: String,
    pub algo: Option<String>,
    #[serde(default)]
    pub size: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// 按 app 过滤（顶层页看全量时不传）
    pub app_id: Option<String>,
}

async fn list_all(
    State(state): State<AppState>,
    Query(filter): Query<ListParams>,
) -> ApiResult<Vec<ChannelView>> {
    let rows = repo::channel::list_all(&state.db).await?;

    let app_id = match &filter.app_id {
        Some(raw) => Some(
            raw.parse::<Uuid>()
                .map_err(|e| AppError::BadRequest(format!("invalid app_id: {e}")))?,
        ),
        None => None,
    };

    Ok(ApiOk(
        rows.into_iter()
            .filter(|row| app_id.is_none_or(|id| row.app_id == id))
            .map(ChannelView::from_row)
            .collect(),
    ))
}

async fn detail(Path(guid): Path<Uuid>, State(state): State<AppState>) -> ApiResult<ChannelView> {
    let row = repo::channel::get(&state.db, guid)
        .await?
        .ok_or_else(|| AppError::NotFound(guid.to_string()))?;

    Ok(ApiOk(ChannelView::from_row(row)))
}

async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateChannelRequest>,
) -> Result<Response, AppError> {
    validate_sha256(&body.latest_sha256)?;

    // 先查后插：app / 资源不存在给 400，而不是撞外键变成 409
    if !repo::app::exists(&state.db, body.app_id).await? {
        return Err(AppError::BadRequest(format!(
            "app {} does not exist",
            body.app_id
        )));
    }
    if !repo::resource::exists(&state.db, &body.latest_sha256).await? {
        return Err(AppError::BadRequest(format!(
            "resource {} does not exist",
            body.latest_sha256
        )));
    }

    let guid = Uuid::new_v4();
    repo::channel::insert(
        &state.db,
        guid,
        body.app_id,
        &body.tag_name,
        &body.latest_version,
        &body.latest_sha256,
        body.is_default,
    )
    .await?;

    let row = repo::channel::get(&state.db, guid)
        .await?
        .ok_or_else(|| AppError::NotFound(guid.to_string()))?;

    Ok(with_status(StatusCode::CREATED, ChannelView::from_row(row)))
}

async fn modify(
    Path(guid): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<UpdateChannelRequest>,
) -> ApiResult<ChannelView> {
    if let Some(sha256) = &body.latest_sha256 {
        validate_sha256(sha256)?;
        if !repo::resource::exists(&state.db, sha256).await? {
            return Err(AppError::BadRequest(format!(
                "resource {sha256} does not exist"
            )));
        }
    }

    if !repo::channel::update(
        &state.db,
        guid,
        body.tag_name.as_deref(),
        body.latest_version.as_deref(),
        body.latest_sha256.as_deref(),
        body.is_default,
    )
    .await?
    {
        return Err(AppError::NotFound(guid.to_string()));
    }

    let row = repo::channel::get(&state.db, guid)
        .await?
        .ok_or_else(|| AppError::NotFound(guid.to_string()))?;

    Ok(ApiOk(ChannelView::from_row(row)))
}

async fn remove(Path(guid): Path<Uuid>, State(state): State<AppState>) -> ApiResult<Uuid> {
    if !repo::channel::delete(&state.db, guid).await? {
        return Err(AppError::NotFound(guid.to_string()));
    }

    Ok(ApiOk(guid))
}

#[derive(Debug, Serialize)]
pub struct ReleaseView {
    pub version: String,
    pub sha256: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_releases(
    Path(guid): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<Vec<ReleaseView>> {
    let rows = repo::channel::list_releases(&state.db, guid).await?;

    Ok(ApiOk(
        rows.into_iter()
            .map(|row| ReleaseView {
                version: row.version,
                sha256: row.sha256,
                created_at: row.created_at,
            })
            .collect(),
    ))
}

async fn create_release(
    Path(guid): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<CreateReleaseRequest>,
) -> Result<Response, AppError> {
    validate_sha256(&body.sha256)?;
    ensure_channel(&state, guid).await?;
    if !repo::resource::exists(&state.db, &body.sha256).await? {
        return Err(AppError::BadRequest(format!(
            "resource {} does not exist",
            body.sha256
        )));
    }

    repo::channel::insert_release(&state.db, guid, &body.version, &body.sha256).await?;

    Ok(with_status(
        StatusCode::CREATED,
        ReleaseView {
            version: body.version,
            sha256: body.sha256,
            created_at: chrono::Utc::now(),
        },
    ))
}

async fn delete_release(
    Path((guid, version)): Path<(Uuid, String)>,
    State(state): State<AppState>,
) -> ApiResult<String> {
    if !repo::channel::delete_release(&state.db, guid, &version).await? {
        return Err(AppError::NotFound(version));
    }

    Ok(ApiOk(version))
}

#[derive(Debug, Serialize)]
pub struct DiffView {
    pub base_sha256: String,
    pub patch_sha256: String,
    pub algo: Option<String>,
    pub size: i64,
}

async fn list_diffs(
    Path(guid): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<Vec<DiffView>> {
    let rows = repo::channel::list_diffs(&state.db, guid).await?;

    Ok(ApiOk(
        rows.into_iter()
            .map(|row| DiffView {
                base_sha256: row.base_sha256,
                patch_sha256: row.patch_sha256,
                algo: row.algo,
                size: row.size,
            })
            .collect(),
    ))
}

async fn create_diff(
    Path(guid): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<CreateDiffRequest>,
) -> Result<Response, AppError> {
    validate_sha256(&body.base_sha256)?;
    validate_sha256(&body.patch_sha256)?;
    if body.base_sha256 == body.patch_sha256 {
        return Err(AppError::BadRequest(
            "base_sha256 与 patch_sha256 不能相同".into(),
        ));
    }
    ensure_channel(&state, guid).await?;
    for sha256 in [&body.base_sha256, &body.patch_sha256] {
        if !repo::resource::exists(&state.db, sha256).await? {
            return Err(AppError::BadRequest(format!(
                "resource {sha256} does not exist"
            )));
        }
    }

    repo::channel::insert_diff(
        &state.db,
        guid,
        &body.base_sha256,
        &body.patch_sha256,
        body.algo.as_deref(),
        body.size,
    )
    .await?;

    Ok(with_status(
        StatusCode::CREATED,
        DiffView {
            base_sha256: body.base_sha256,
            patch_sha256: body.patch_sha256,
            algo: body.algo,
            size: body.size,
        },
    ))
}

async fn delete_diff(
    Path((guid, base_sha256)): Path<(Uuid, String)>,
    State(state): State<AppState>,
) -> ApiResult<String> {
    validate_sha256(&base_sha256)?;
    if !repo::channel::delete_diff(&state.db, guid, &base_sha256).await? {
        return Err(AppError::NotFound(base_sha256));
    }

    Ok(ApiOk(base_sha256))
}

fn validate_sha256(sha256: &str) -> Result<(), AppError> {
    object_key(sha256)
        .map(|_| ())
        .map_err(|e| AppError::BadRequest(e.to_string()))
}

/// 通道不存在时 404（release / diff 都挂在通道下）。
async fn ensure_channel(state: &AppState, guid: Uuid) -> Result<(), AppError> {
    if repo::channel::get(&state.db, guid).await?.is_none() {
        return Err(AppError::NotFound(guid.to_string()));
    }
    Ok(())
}

impl ChannelView {
    fn from_row(row: repo::channel::ChannelRow) -> Self {
        Self {
            guid: row.guid,
            app_id: row.app_id,
            tag_name: row.tag_name,
            latest_version: row.latest_version,
            raw_sha256: row.raw_sha256,
            raw_size: row.raw_size as u64,
            is_default: row.is_default,
        }
    }
}
