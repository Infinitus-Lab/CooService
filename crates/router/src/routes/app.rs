//! `/api/v1/app`：客户端接口，app 由 `X-App-Id` 头指定。

use axum::{
    Router,
    extract::{Path, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use database::repo;
use serde::Serialize;
use uuid::Uuid;

use crate::{ApiOk, ApiResult, AppError, extract::AppId, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/channels", get(list_channels))
        .route("/channels/{guid}", get(get_update))
        .route("/announces", get(list_announces))
}

#[derive(Debug, Serialize)]
pub struct ChannelView {
    pub guid: Uuid,
    pub tag_name: String,
    pub latest_version: String,
    pub raw_sha256: String,
    pub raw_size: u64,
    /// 客户端优先选这个；没有就取列表第一个
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
pub struct DiffView {
    pub base_sha256: String,
    pub patch_sha256: String,
    pub algo: Option<String>,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct UpdateView {
    pub guid: Uuid,
    pub tag_name: String,
    pub version: String,
    pub raw_sha256: String,
    pub raw_size: u64,
    /// 客户端拿本地版本的 sha256 比对，命中就下补丁，否则下 raw 包
    pub diffs: Vec<DiffView>,
}

async fn list_channels(
    AppId(app_id): AppId,
    State(state): State<AppState>,
) -> ApiResult<Vec<ChannelView>> {
    let channels = state.apps.channels(app_id).await?;

    Ok(ApiOk(
        channels
            .into_iter()
            .map(|channel| ChannelView {
                guid: channel.guid,
                tag_name: channel.tag_name,
                latest_version: channel.latest_version,
                raw_sha256: channel.raw_sha256,
                raw_size: channel.raw_size,
                is_default: channel.is_default,
            })
            .collect(),
    ))
}

/// 自己解析 guid，用 `Path<Uuid>` 的话解析失败会落到 axum 默认拒绝，返回纯文本而非统一信封。
async fn get_update(
    Path(guid): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<UpdateView> {
    let guid: Uuid = guid
        .parse()
        .map_err(|_| AppError::BadRequest(format!("invalid channel guid: {guid}")))?;

    let update = state
        .apps
        .update(guid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("channel {guid}")))?;

    Ok(ApiOk(UpdateView {
        guid: update.channel.guid,
        tag_name: update.channel.tag_name,
        version: update.channel.latest_version,
        raw_sha256: update.channel.raw_sha256,
        raw_size: update.channel.raw_size,
        diffs: update
            .diffs
            .into_iter()
            .map(|diff| DiffView {
                base_sha256: diff.base_sha256,
                patch_sha256: diff.patch_sha256,
                algo: diff.algo,
                size: diff.size,
            })
            .collect(),
    }))
}

// ---------- 公告（客户端只读） ----------

#[derive(Debug, Serialize)]
pub struct AnnounceView {
    pub guid: Uuid,
    pub title: String,
    pub content: String,
    /// 展示起点，NULL 不限
    pub starts_at: Option<DateTime<Utc>>,
    /// 展示终点，NULL 不限
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn list_announces(
    AppId(app_id): AppId,
    State(state): State<AppState>,
) -> ApiResult<Vec<AnnounceView>> {
    // 与 channels 一致：app 不存在或已禁用时 404
    if state.apps.get(app_id).is_none() {
        return Err(AppError::NotFound(app_id.to_string()));
    }

    // 客户端只看当前可见的：起点已到、终点未过（NULL 边不限）
    let rows = repo::announce::list_visible_by_app(&state.db, app_id).await?;
    Ok(ApiOk(
        rows.into_iter()
            .map(|row| AnnounceView {
                guid: row.guid,
                title: row.title,
                content: row.content,
                starts_at: row.starts_at,
                expires_at: row.expires_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect(),
    ))
}
