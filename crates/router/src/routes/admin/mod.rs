//! `/api/v1/admin`：管理接口统一放在本目录。
//!
//! 整棵子树要求 `X-Admin-Key`，密钥每次启动随机生成（见 `crate::auth`）。
//! 约定：本目录只放 admin 接口；公开接口在 `routes/app.rs` 与 `routes/resources.rs`，
//! 不混写——避免未鉴权路由被挂进 admin 子树，或 admin 能力漏到公开侧。

pub mod announces;
pub mod apps;
pub mod channels;
pub mod pools;
pub mod resources;

use axum::{
    Router, extract::State, middleware, routing::post,
};

use crate::{ApiOk, ApiResult, auth::require_admin_key, pools::reload_resource_pools, state::AppState};

pub fn router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/reload", post(reload))
        .nest("/apps", apps::router())
        .nest("/pools", pools::router())
        .nest("/resources", resources::admin_router())
        .nest("/channels", channels::router())
        .nest("/announces", announces::router())
        // from_fn_with_state：中间件要读 AppState 里的密钥
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_admin_key,
        ))
}

/// 重扫 `app` 表与资源池，让外部改动生效；部分失败返回 500 并记日志。
async fn reload(State(state): State<AppState>) -> ApiResult<String> {
    if let Err(e) = state.apps.reload().await {
        tracing::error!(error = %e, "reload apps failed");
        return Err(crate::AppError::Internal(anyhow::anyhow!("reload apps failed: {e}")));
    }
    if let Err(e) = reload_resource_pools(&state.resources, &state.pool_meta, &state.db).await {
        tracing::error!(error = %e, "reload pools failed");
        return Err(crate::AppError::Internal(anyhow::anyhow!("reload pools failed: {e}")));
    }
    // reload 后新池重新注册，让同步引擎补上实有集
    state.sync.notify();

    Ok(ApiOk(format!(
        "reloaded {} apps, {} pools",
        state.apps.count(),
        state.resources.count()
    )))
}