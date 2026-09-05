//! `/api/v1`：客户端接口（`app`）与管理接口（`admin`）。
//!
//! 下载子树（`resources`）并入同一 `/v1` 树；下载是 302 瞬时响应，统一超时窗口足够（见 `crate::router`）。

use axum::Router;

use super::{admin, app};
use crate::state::AppState;

pub fn router(state: &AppState) -> Router<AppState> {
    Router::new()
        .nest("/app", app::router())
        .nest("/admin", admin::router(state))
}
