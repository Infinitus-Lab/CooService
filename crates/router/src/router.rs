//! 路由树与中间件装配。

use std::time::Duration;

use axum::{Router, http::StatusCode};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::{
    error::not_found,
    routes::{self, health::health},
    state::AppState,
};

/// 中间件由外到内：CORS → Timeout → Trace（ServiceBuilder 后加的层在最外层）。
/// 外层再套一个 CORS：让出错分支（超时/路由 fallback）也带跨域头，WebUI 才能读错误信封。
/// 服务端不代理流转发（下载是 302 重定向，瞬时响应），无需给子树豁免超时。
pub fn build_router(state: AppState, request_timeout: Duration) -> Router {
    let timed = routes::v1::router(&state)
        .merge(routes::resources::router())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    request_timeout,
                ))
                // WebUI 独立部署，需跨域
                .layer(CorsLayer::permissive()),
        );

    Router::new()
        .route("/healthz", axum::routing::get(health))
        .nest("/api", Router::new().nest("/v1", timed))
        .fallback(not_found)
        .layer(CorsLayer::permissive())
        .with_state(state)
}
