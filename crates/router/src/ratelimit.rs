//! 公开端点的进程内限流：固定窗口，按客户端 IP 计数。
//!
//! 只防"洪水打满 DB 连接池拖垮服务"这类粗暴攻击；精细限流应交外围网关。
//! 直连部署下取 TCP 源地址；有 `X-Forwarded-For` 时取首个地址（直连场景该头并不可信，
//! 欺骗只会绕开自己的限流，不影响服务可用性）。

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;

/// 每窗口每 IP 的计数。
struct Bucket {
    count: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    buckets: Arc<DashMap<IpAddr, Bucket>>,
}

impl RateLimiter {
    pub fn new(limit: u32) -> Self {
        Self {
            limit,
            window: Duration::from_secs(60),
            buckets: Arc::new(DashMap::new()),
        }
    }

    /// 是否放行；过期窗口惰性重置。
    fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.buckets.entry(ip).or_insert_with(|| Bucket {
            count: 0,
            window_start: now,
        });

        if now.duration_since(entry.window_start) >= self.window {
            entry.count = 0;
            entry.window_start = now;
        }
        entry.count += 1;
        entry.count <= self.limit
    }
}

/// 中间件：公开路由全局共享进程内的 `Arc<RateLimiter>`。
///
/// ConnectInfo 提取器（Rejection 非 IntoResponse）不能用于 `middleware::from_fn`，
/// 这里直接从 extensions 里取——serve 时必须用 `into_make_service_with_connect_info`。
pub async fn ratelimit(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Response {
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .or_else(|| {
            request
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .and_then(|ip| ip.trim().parse::<IpAddr>().ok())
        });

    if ip.is_some_and(|ip| !limiter.check(ip)) {
        return (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_resets_and_counts() {
        let limiter = RateLimiter::new(2);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(limiter.check(ip));
        assert!(limiter.check(ip));
        assert!(!limiter.check(ip));
        // 新窗口
        limiter.buckets.get_mut(&ip).unwrap().window_start = Instant::now() - Duration::from_secs(61);
        assert!(limiter.check(ip));
    }
}