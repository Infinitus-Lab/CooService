//! 管理接口鉴权。
//!
//! 密钥来源：`ADMIN_KEY` 环境变量（运维密管，可持久化）。未设置时每次启动
//! 随机生成 64 字节密钥——注意该密钥**不会**完整打印，仅打前 8 位指纹，运维
//! 无法从日志取回，正式部署必须显式注入 `ADMIN_KEY`。

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

/// 常时比较：长度差异折进累计值，比较耗时与内容无关（防响应时间泄露字节信息）。
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    let max = left.len().max(right.len());
    for i in 0..max {
        let l = left.get(i).copied().unwrap_or(0);
        let r = right.get(i).copied().unwrap_or(0);
        diff |= (l ^ r) as usize;
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
