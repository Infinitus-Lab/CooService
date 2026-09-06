//! 路由树与中间件装配。
//!
//! 超时与请求体上限按分支配置：
//!   · 常规树（/api/v1 除上传）套 `REQUEST_TIMEOUT_SECS` 超时
//!   · 上传是慢链路大文件流式写盘，套 `MAX_UPLOAD_BYTES` 上限但**豁免超时**，
//!     否则慢客户端传大文件必被 `TimeoutLayer` 408 腰斩
//! CORS 双层（内层业务响应 + 外层错误分支）共用同一份 Origin 白名单配置。

use std::time::Duration;

use axum::{
    Router,
    http::{
        HeaderName, HeaderValue, Method, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN},
    },
    middleware,
    routing::put,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{
    auth::require_admin_key,
    error::not_found,
    routes::{self, admin, health::health},
    state::AppState,
};

/// 中间件由外到内：CORS → Timeout → Trace（ServiceBuilder 后加的层在最外层）。
/// 外层再套一个 CORS：让出错分支（超时/路由 fallback）也带跨域头，WebUI 才能读错误信封。
/// 服务端不代理流转发（下载是 302 重定向，瞬时响应），无需给子树豁免超时。
pub fn build_router(
    state: AppState,
    request_timeout: Duration,
    cors_allowed_origins: Vec<String>,
) -> Router {
    let cors = cors_layer(&cors_allowed_origins);

    // 限流只挂公开子树（无认证的 DB 防护）；admin 子树有密钥门，不需要
    let public = routes::app::router()
        .merge(routes::resources::router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::ratelimit::ratelimit,
        ));

    let timed = routes::v1::router(&state)
        .merge(public)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    request_timeout,
                ))
                .layer(cors.clone()),
        );

    // 上传分支：body 上限显式生效（Body 提取器不吃 DefaultBodyLimit，需 RequestBodyLimitLayer），
    // 且不受 30s 超时约束——大文件慢链路上传由大小上限兜底，超时由读写层自行处理。
    let upload = Router::new()
        .route("/admin/resources", put(admin::resources::upload))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(
            state.request_max_upload_bytes as usize,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_admin_key,
        ));

    Router::new()
        .route("/healthz", axum::routing::get(health))
        .nest("/api", Router::new().nest("/v1", timed.merge(upload)))
        .fallback(not_found)
        .layer(cors)
        .with_state(state)
}

/// 按配置构 CORS 层：白名单为空时 permissive（内网开发），否则只放行列入的 Origin，
/// 同时放行管理密钥头与 302 所需方法。
fn cors_layer(allowed: &[String]) -> CorsLayer {
    if allowed.is_empty() {
        return CorsLayer::permissive();
    }

    let origins: Vec<HeaderValue> = allowed
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([
            ORIGIN,
            ACCEPT,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_static("x-admin-key"),
            HeaderName::from_static("x-app-id"),
        ])
        .allow_credentials(false)
}