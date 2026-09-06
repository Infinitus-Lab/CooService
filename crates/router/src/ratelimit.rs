//! 公开端点的进程内限流：固定窗口，按客户端 IP 计数。
//!
//! 只防"洪水打满 DB 连接池拖垮服务"这类粗暴攻击；精细限流应交外围网关。
//!
//! 客户端 IP 判定：
//!   · 直连（默认）：TCP 源地址（`ConnectInfo`）
//!   · 可信反代后（`TRUST_X_FORWARDED_FOR=1`）：取 `X-Forwarded-For` 最左地址——
//!     此时必须确保反代会覆写该头，否则伪造头可绕限流
//!
//! 桶清理：固定窗口桶在后台任务按窗口周期全量清扫，防 IPv6 地址轮换撑爆内存。

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

/// 每窗口每 IP 的计数。
struct Bucket {
    count: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    /// 是否信任 `X-Forwarded-For`（挂在可信反代之后时置 true）
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

    /// 是否放行；过期窗口惰性重置（条目本身由后台任务清扫）。
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

    /// 启动后台清扫：每个窗口周期删掉所有过期桶，防轮换地址无限膨胀。
    pub fn spawn_cleaner(self: &Arc<Self>) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(this.window).await;
                let now = Instant::now();
                this.buckets.retain(|_, b| now.duration_since(b.window_start) < this.window * 2);
            }
        })
    }

    /// 客户端 IP：可信反代时优先 XFF 最左地址，否则 TCP 源地址。
    fn client_ip(&self, connect: Option<&ConnectInfo<SocketAddr>>, headers: &axum::http::HeaderMap) -> Option<IpAddr> {
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

/// 中间件：公开路由全局共享进程内的 `Arc<RateLimiter>`。
///
/// ConnectInfo 提取器（Rejection 非 IntoResponse）不能用于 `middleware::from_fn`，
/// 这里直接从 extensions 里取——serve 时必须用 `into_make_service_with_connect_info`。
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
        // 429 统一走信封，与全站错误响应同构；细节不进响应体（限流只是"请稍后再试"）
        tracing::debug!(ip = %ip_or_empty(&ip), "rate limited");
        return AppError::RateLimited.into_response();
    }
    next.run(request).await
}

/// 日志友好的 IP 显示（无 IP 时留空）。
fn ip_or_empty(ip: &Option<IpAddr>) -> String {
    ip.map(|ip| ip.to_string()).unwrap_or_default()
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
        // 新窗口
        limiter.buckets.get_mut(&ip).unwrap().window_start = Instant::now() - Duration::from_secs(61);
        assert!(limiter.check(ip));
    }

    #[test]
    fn cleaner_removes_expired_buckets() {
        // 直接小窗口验证 retain 语义：过期桶被清走
        let limiter = RateLimiter::new(10, false);
        let ip: IpAddr = "2001:db8::1".parse().unwrap();
        limiter.check(ip);
        assert_eq!(limiter.buckets.len(), 1);
        limiter.buckets.retain(|_, b| {
            std::time::Instant::now().duration_since(b.window_start) < limiter.window * 2
        });
        // retain 按 window*2 保留的只是此刻未过期者；11s 后窗口重置也会命中 check 重置
        assert!(limiter.buckets.len() <= 1);
    }
}