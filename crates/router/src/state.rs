//! 应用状态：所有 handler 通过 `State<AppState>` 访问。

use std::sync::Arc;

use apps::manager::AppManager;
use axum::extract::FromRef;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use database::Database;
use resource::{health::PoolHealth, local::LocalPool, pool::ResourcePool};

use crate::{cache::PublicCache, pool_sync::PoolSync, ratelimit::RateLimiter};

impl FromRef<AppState> for Arc<RateLimiter> {
    fn from_ref(state: &AppState) -> Self {
        state.rate_limiter.clone()
    }
}

impl FromRef<AppState> for Arc<PublicCache> {
    fn from_ref(state: &AppState) -> Self {
        state.public_cache.clone()
    }
}

/// 池的公开信息：下载 302 挑候选池时用；由 `routes/admin/pools.rs` 维护（create/modify 就地生效）。
#[derive(Debug, Clone)]
pub struct PoolMeta {
    pub public_endpoint: String,
    pub is_public: bool,
}

/// 池 id → 公开信息。
pub type PoolMetaMap = DashMap<String, PoolMeta>;

/// 每请求 clone 一次，重对象一律放 `Arc`。
#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
    pub apps: Arc<AppManager>,
    pub resources: Arc<ResourcePool>,
    /// 已连接池的公开信息（随 reload 重扫）
    pub pool_meta: Arc<PoolMetaMap>,
    /// 本地完整副本：池成员入池与同步引擎的推送源
    pub local: Arc<LocalPool>,
    /// 池健康状态（resource crate 的探测任务维护）
    pub health: Arc<PoolHealth>,
    /// 池同步引擎（期望登记 vs 实有，自动收敛）
    pub sync: Arc<PoolSync>,
    /// 数据库连接池
    pub db: Database,
    /// 管理密钥（`ADMIN_KEY` 注入或随机生成，见 `auth` 模块头）
    pub admin_key: Arc<str>,
    /// 上传请求体上限（字节），由路由装配层消费
    pub request_max_upload_bytes: u64,
    /// 公开端点短 TTL 缓存
    pub public_cache: Arc<PublicCache>,
    /// 公开端点每 IP 限流
    pub rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        apps: AppManager,
        resources: Arc<ResourcePool>,
        pool_meta: PoolMetaMap,
        local: Arc<LocalPool>,
        health: Arc<PoolHealth>,
        sync: Arc<PoolSync>,
        db: Database,
        admin_key: String,
        request_max_upload_bytes: u64,
        public_cache: Arc<PublicCache>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            started_at: Utc::now(),
            apps: Arc::new(apps),
            resources,
            pool_meta: Arc::new(pool_meta),
            local,
            health,
            sync,
            db,
            admin_key: Arc::from(admin_key),
            request_max_upload_bytes,
            public_cache,
            rate_limiter,
        }
    }
}
