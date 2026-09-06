//! 统一错误类型与 HTTP 映射。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::response::Envelope;

/// handler 统一返回类型：`Ok` 包成 [`crate::ApiOk`]，`Err` 由 [`AppError`] 转 HTTP 响应。
pub type ApiResult<T = ()> = Result<crate::response::ApiOk<T>, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    /// 上游（资源池 / 数据库）短暂不可用
    #[error("bad gateway: {0}")]
    BadGateway(String),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl From<database::DatabaseError> for AppError {
    fn from(e: database::DatabaseError) -> Self {
        use database::DatabaseError as Db;

        // 外键 / 唯一约束违反通常是业务规则（重复、被引用），统一按冲突处理
        match e {
            Db::Constraint { .. } => Self::Conflict(e.to_string()),
            other => Self::Internal(anyhow::Error::new(other)),
        }
    }
}

/// 领域层错误 → HTTP 错误：数据库故障归 500，其余按语义映射。
impl From<apps::error::AppError> for AppError {
    fn from(e: apps::error::AppError) -> Self {
        use apps::error::AppError as Domain;

        match e {
            Domain::Database(e) => Self::Internal(anyhow::Error::new(e)),
            Domain::NotFound(id) => Self::NotFound(id),
            Domain::AlreadyExists(id) => Self::Conflict(id),
        }
    }
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::BadGateway(_) => StatusCode::BAD_GATEWAY,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();

        // 5xx 是服务端故障，记 error；4xx 是客户端问题，只记 debug
        if status.is_server_error() {
            tracing::error!(error = %self, status = %status, "request failed");
        } else {
            tracing::debug!(error = %self, status = %status, "request rejected");
        }

        // 5xx 的完整细节只进日志，响应体用固定文案，不把 DB 报错 / 内部路径外泄
        let message = if status.is_server_error() {
            "internal error".to_string()
        } else {
            self.to_string()
        };

        (status, Json(Envelope::message(status, message))).into_response()
    }
}

/// 未匹配路由的 fallback，响应体结构与业务错误一致。
pub async fn not_found() -> Response {
    let status = StatusCode::NOT_FOUND;
    (status, Json(Envelope::message(status, "route not found"))).into_response()
}
