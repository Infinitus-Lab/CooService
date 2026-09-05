//! 统一 JSON 响应封装。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Serialize;

/// 所有接口返回体的外层结构，成功失败同构。
#[derive(Debug, Clone, Serialize)]
pub struct Envelope<T> {
    /// 与 HTTP 状态码一致
    pub code: u16,
    pub message: String,
    /// 失败时为 null
    pub data: Option<T>,
    /// RFC3339
    pub timestamp: String,
}

impl<T> Envelope<T> {
    pub fn new(code: StatusCode, message: impl Into<String>, data: Option<T>) -> Self {
        Self {
            code: code.as_u16(),
            message: message.into(),
            data,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    pub fn ok(data: T) -> Self {
        Self::new(StatusCode::OK, "ok", Some(data))
    }
}

/// `data` 为 `null` 的响应（错误、纯状态返回）。
impl Envelope<()> {
    pub fn message(code: StatusCode, message: impl Into<String>) -> Self {
        Self::new(code, message, None)
    }
}

/// 成功响应包装：handler 返回 `Ok(ApiOk(data))` 时输出 `{ code: 200, data }`。
#[derive(Debug, Clone)]
pub struct ApiOk<T>(pub T);

impl<T: Serialize> IntoResponse for ApiOk<T> {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(Envelope::ok(self.0))).into_response()
    }
}

/// 带自定义状态码的成功响应，如 201 Created。
pub fn with_status<T: Serialize>(status: StatusCode, data: T) -> Response {
    (
        status,
        Json(Envelope::new(
            status,
            status.canonical_reason().unwrap_or("ok"),
            Some(data),
        )),
    )
        .into_response()
}
