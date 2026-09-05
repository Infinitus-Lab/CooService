//! 管理接口鉴权。
//!
//! 密钥每次启动随机生成，只记在日志里，不落盘、不进数据库。

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
};
use rand::{TryRng, rngs::SysRng};

use crate::AppError;

pub const ADMIN_KEY_HEADER: &str = "x-admin-key";

/// 每次启动生成 64 字节随机密钥，十六进制后 128 字符。
pub fn generate_admin_key() -> String {
    let mut bytes = [0u8; 64];
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("os rng unavailable");
    hex_encode(&bytes)
}

/// 挂在 `/api/v1/admin` 上，缺密钥或不匹配一律 401。
pub async fn require_admin_key(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let provided = headers
        .get(ADMIN_KEY_HEADER)
        .and_then(|value| value.to_str().ok());

    match provided {
        Some(key) if constant_time_eq(key.as_bytes(), state.admin_key.as_bytes()) => {
            next.run(request).await
        }
        Some(_) => AppError::Unauthorized("invalid admin key".into()).into_response(),
        None => {
            AppError::Unauthorized(format!("missing {ADMIN_KEY_HEADER} header")).into_response()
        }
    }
}

/// 定长比较：不因首字节不同就提前返回，避免用响应时间逐字节猜密钥。
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}
