//! `/api/v1/resources`：客户端下载入口（公开、无鉴权）。
//!
//! 服务端不代理流转发——下载只有一条 200 路径：**302 重定向到公开池**，
//! 所有其余场景一律 503（无可用存储池 / 资源未入库 / 无公开池）。
//! 管理接口在 `admin/resources.rs`，本文件只放公开接口。

use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use database::repo;
use resource::key::object_key;
use uuid::Uuid;

use crate::{AppError, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/resources/{sha256}", get(fetch))
}

async fn fetch(
    Path(sha256): Path<String>,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    let key = object_key(&sha256).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let row = repo::resource::get(&state.db, &sha256)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("resource {sha256}")))?;

    // 没有任何可用的存储池 → 服务整体不可用
    if state.resources.count() == 0 {
        return Err(AppError::ServiceUnavailable(
            "no storage pool available, download unavailable".into(),
        ));
    }

    // 内容没进过任何池（仅本地）→ 没有可重定向的源头
    if row.pools.is_empty() {
        return Err(AppError::ServiceUnavailable(format!(
            "resource {sha256} is not stored in any pool"
        )));
    }

    // 候选池：公开、连接还在、且健康探测通过；没有 → 无法对外提供下载
    let candidates: Vec<&str> = row
        .pools
        .iter()
        .filter(|id| {
            state.pool_meta.get(*id).is_some_and(|m| m.is_public)
                && state.resources.get(id).is_some()
                && state.health.status(id).is_some_and(|h| h.up)
        })
        .map(String::as_str)
        .collect();

    let Some(pool_id) = pick(candidates.as_slice()) else {
        return Err(AppError::ServiceUnavailable(
            "no public storage pool available, download unavailable".into(),
        ));
    };

    let redirect = {
        let meta = state.pool_meta.get(pool_id).expect("candidate in meta");
        format!("{}/{key}", meta.public_endpoint.trim_end_matches('/'))
    };
    tracing::info!(sha256, pool = pool_id, %redirect, "download redirect");
    Ok((StatusCode::FOUND, [(header::LOCATION, redirect)]).into_response())
}

/// 从候选里均匀随机挑一个：uuid 当随机源，避免多拉一个依赖。
fn pick<'a>(candidates: &'a [&str]) -> Option<&'a str> {
    if candidates.is_empty() {
        return None;
    }

    let index = (Uuid::new_v4().as_u128() % candidates.len() as u128) as usize;
    Some(candidates[index])
}
