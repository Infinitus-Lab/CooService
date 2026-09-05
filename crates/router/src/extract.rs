//! 请求头提取器。

use axum::{extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

use crate::AppError;

/// `X-App-Id: <uuid>`，客户端用它指定要访问哪个 app。
pub struct AppId(pub Uuid);

impl<S> FromRequestParts<S> for AppId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get("x-app-id")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::BadRequest("missing X-App-Id header".into()))?;

        let guid = Uuid::parse_str(raw)
            .map_err(|_| AppError::BadRequest(format!("invalid X-App-Id header: {raw}")))?;

        Ok(Self(guid))
    }
}
