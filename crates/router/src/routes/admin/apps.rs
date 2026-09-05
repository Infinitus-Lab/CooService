//! `/api/v1/admin/apps`：app 的增删改查，全部落在 `app` 表。
//!
//! 读走 `repo::app`（能拿到 enabled），写走 `AppManager`（并同步内存缓存）。

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::{delete, get},
};
use database::repo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ApiOk, ApiResult, AppError, response::with_status, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).patch(modify).delete(remove))
        .route("/{id}/resources", get(list_resources).post(link_resource))
        .route("/{id}/resources/{sha256}", delete(unlink_resource))
        .route("/{id}/announces", get(list_announces).post(link_announce))
        .route("/{id}/announces/{guid}", delete(unlink_announce))
}

#[derive(Debug, Serialize)]
pub struct AppView {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateAppRequest {
    /// 不传就由服务端生成 uuid v4
    pub id: Option<Uuid>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAppRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
}

async fn list(State(state): State<AppState>) -> ApiResult<Vec<AppView>> {
    let rows = repo::app::list_all(&state.db).await?;

    Ok(ApiOk(
        rows.into_iter()
            .map(|row| AppView {
                id: row.id,
                name: row.name,
                enabled: row.enabled,
            })
            .collect(),
    ))
}

async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateAppRequest>,
) -> Result<Response, AppError> {
    let id = body.id.unwrap_or_else(Uuid::new_v4);
    state.apps.create(id, &body.name).await?;

    Ok(with_status(
        StatusCode::CREATED,
        AppView {
            id,
            name: body.name,
            enabled: true,
        },
    ))
}

async fn detail(Path(id): Path<Uuid>, State(state): State<AppState>) -> ApiResult<AppView> {
    let row = repo::app::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(id.to_string()))?;

    Ok(ApiOk(AppView {
        id: row.id,
        name: row.name,
        enabled: row.enabled,
    }))
}

async fn modify(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<UpdateAppRequest>,
) -> ApiResult<AppView> {
    state
        .apps
        .set_info(id, body.name.as_deref(), body.enabled)
        .await?;

    let row = repo::app::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(id.to_string()))?;

    Ok(ApiOk(AppView {
        id: row.id,
        name: row.name,
        enabled: row.enabled,
    }))
}

async fn remove(Path(id): Path<Uuid>, State(state): State<AppState>) -> ApiResult<Uuid> {
    let app = state.apps.remove(id).await?;
    Ok(ApiOk(app.id))
}

// ---------- app 的资源关联 ----------

#[derive(Debug, Serialize)]
pub struct AppResourceView {
    pub sha256: String,
    pub size: i64,
}

#[derive(Debug, Deserialize)]
pub struct LinkResourceRequest {
    pub sha256: String,
}

async fn ensure_app_exists(db: &database::Database, id: Uuid) -> Result<(), AppError> {
    if !repo::app::exists(db, id).await? {
        return Err(AppError::NotFound(id.to_string()));
    }
    Ok(())
}

async fn list_resources(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<Vec<AppResourceView>> {
    ensure_app_exists(&state.db, id).await?;

    let rows = repo::resource::list_app_resources(&state.db, id).await?;
    Ok(ApiOk(
        rows.into_iter()
            .map(|row| AppResourceView {
                sha256: row.sha256,
                size: row.size,
            })
            .collect(),
    ))
}

async fn link_resource(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<LinkResourceRequest>,
) -> Result<Response, AppError> {
    resource::key::object_key(&body.sha256).map_err(|e| AppError::BadRequest(e.to_string()))?;
    ensure_app_exists(&state.db, id).await?;

    // 先查后插：资源不存在时给 400，而不是撞 FK 变 500
    let row = repo::resource::get(&state.db, &body.sha256)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("resource {} does not exist", body.sha256)))?;

    repo::resource::link_app_resource(&state.db, id, &body.sha256).await?;

    Ok(with_status(
        StatusCode::CREATED,
        AppResourceView {
            sha256: body.sha256,
            size: row.size,
        },
    ))
}

async fn unlink_resource(
    Path((id, sha256)): Path<(Uuid, String)>,
    State(state): State<AppState>,
) -> ApiResult<String> {
    resource::key::object_key(&sha256).map_err(|e| AppError::BadRequest(e.to_string()))?;
    ensure_app_exists(&state.db, id).await?;

    if !repo::resource::unlink_app_resource(&state.db, id, &sha256).await? {
        return Err(AppError::NotFound(sha256.clone()));
    }

    Ok(ApiOk(sha256))
}

// ---------- app 的公告关联 ----------

use super::announces::AnnounceView;

#[derive(Debug, Deserialize)]
pub struct LinkAnnounceRequest {
    pub guid: Uuid,
}

async fn list_announces(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<Vec<AnnounceView>> {
    ensure_app_exists(&state.db, id).await?;

    let rows = repo::announce::list_by_app(&state.db, id).await?;
    Ok(ApiOk(
        rows.into_iter().map(AnnounceView::from_row).collect(),
    ))
}

async fn link_announce(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(body): Json<LinkAnnounceRequest>,
) -> Result<Response, AppError> {
    ensure_app_exists(&state.db, id).await?;

    // 先查后插：公告不存在给 400，而不是撞外键
    let row = repo::announce::get(&state.db, body.guid)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("announce {} does not exist", body.guid)))?;

    repo::announce::link(&state.db, id, body.guid).await?;

    Ok(with_status(
        StatusCode::CREATED,
        AnnounceView::from_row(row),
    ))
}

async fn unlink_announce(
    Path((id, guid)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
) -> ApiResult<Uuid> {
    ensure_app_exists(&state.db, id).await?;

    if !repo::announce::unlink(&state.db, id, guid).await? {
        return Err(AppError::NotFound(guid.to_string()));
    }

    Ok(ApiOk(guid))
}
