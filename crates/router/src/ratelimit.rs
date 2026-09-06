//! 公开端点的进程内限流：固定窗口，按客户端 IP 计数。
//!
//! 只防"洪水打满 DB 连接池拖垮服务"这类粗暴攻击；精细限流应交外围网关。
//!
//! 客户端 IP 判定：
//!   · 直连（默认）：TCP 源地址（`ConnectInfo`）
//!   · 可信反代后（`TRUST_X_FORWARDED_FOR=1`）：取 `X-Forwarded-For` 最左地址——
//!     此时必须确保反代会覆写该头，否则伪造头可绕限流
//!
//! 桶由后台任务按窗口周期全量清扫，防 IPv6 地址轮换让桶表无限膨胀
//! （`check` 里的窗口重置只是惰性计数，不负责回收条目）。

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use tokio::task::JoinHandle;

use crate::AppError;

struct Bucket {
    count: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    trust_forwarded: bool,
    buckets: Arc<DashMap<IpAddr, Bucket>>,
}

impl RateLimiter {
    pub fn new(limit: u32, trust_forwarded: bool) -> Self {
        Self {
            limit,
            window: Duration::from_secs(60),
            trust_forwarded,
            buckets: Arc::new(DashMap::new()),
        }
    }

    fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.buckets.entry(ip).or_insert_with(|| Bucket {
            count: 0,
            window_start: now,
        });

        // 窗口过期就重置计数；条目本身留给后台清扫回收
        if now.duration_since(entry.window_start) >= self.window {
            entry.count = 0;
            entry.window_start = now;
        }
        entry.count += 1;
        entry.count <= self.limit
    }

    pub fn spawn_cleaner(self: &Arc<Self>) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(this.window).await;
                let now = Instant::now();
                // 保留两个窗口宽限，避免刚重置的桶被误删
                this.buckets.retain(|_, b| now.duration_since(b.window_start) < this.window * 2);
            }
        })
    }

    fn client_ip(
        &self,
        connect: Option<&ConnectInfo<SocketAddr>>,
        headers: &axum::http::HeaderMap,
    ) -> Option<IpAddr> {
        if self.trust_forwarded
            && let Some(via) = headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .and_then(|ip| ip.trim().parse::<IpAddr>().ok())
        {
            return Some(via);
        }
        connect.map(|info| info.0.ip())
    }
}

/// 中间件：限流要求路由装配层用 `into_make_service_with_connect_info` serve，
/// 否则 `ConnectInfo` 不在 extensions 里，取不到客户端 IP 会直接放行。
pub async fn ratelimit(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Response {
    let ip = limiter.client_ip(
        request.extensions().get::<ConnectInfo<SocketAddr>>(),
        request.headers(),
    );

    if ip.is_some_and(|ip| !limiter.check(ip)) {
        tracing::debug!(ip = %ip.map(|ip| ip.to_string()).unwrap_or_default(), "rate limited");
        return AppError::RateLimited.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_resets_and_counts() {
        let limiter = RateLimiter::new(2, false);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(limiter.check(ip));
        assert!(limiter.check(ip));
        assert!(!limiter.check(ip));
        // 模拟进入下一个窗口：计数应重置
        limiter
            .buckets
            .get_mut(&ip)
            .unwrap()
            .window_start = Instant::now() - Duration::from_secs(61);
        assert!(limiter.check(ip));
    }
}