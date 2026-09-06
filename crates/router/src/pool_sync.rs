//! 池同步引擎：以本地完整副本为真相，把各池收敛到 `resource_location` 的期望登记。
//!
//! 模型：
//!   · 期望集 = DB 里 `resource_location`（该池应当有的对象）
//!   · 实有集 = 运行时已知的池内对象（启动全扫描建立，本服务每次写成功乐观更新）
//!   · 缺失 = 期望 − 实有 → 自动从本地副本推送补齐（本地缺失则记 warn，不算操作数）
//!
//! 只补缺失、不清理池内多余对象（实有 ⊇ 期望）：外部手工放入池的杂物不会被删。
//!
//! 启动先全扫描（后台不阻塞 serve），之后监听变动事件做增量对账；
//! 管理接口 `POST /admin/pools/scan` 可强制全扫描。纯本地上传（无池）不产生期望，
//! 引擎不会主动把资源推进任何池——保持"无池 = 不可下载"语义。

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use database::{Database, repo};
use resource::{
    key::object_key,
    local::LocalPool,
    pool::{RemotePool, ResourcePool},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSyncStatus {
    Syncing,
    Synced,
    Error,
}

impl PoolSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Syncing => "syncing",
            Self::Synced => "synced",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolSyncInfo {
    pub status: PoolSyncStatus,
    pub pending: usize,
    pub scanned_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl PoolSyncInfo {
    fn new(status: PoolSyncStatus, pending: usize, scanned_at: Option<DateTime<Utc>>) -> Self {
        Self {
            status,
            pending,
            scanned_at,
            error: None,
        }
    }
}

pub struct PoolSync {
    db: Database,
    pools: Arc<ResourcePool>,
    local: Arc<LocalPool>,
    states: DashMap<String, PoolSyncInfo>,
    /// 每池已知的实有对象集合（全扫描建立，写入成功乐观更新）
    known: DashMap<String, HashSet<String>>,
    /// 变动通知（broadcast：任意数量接收者都能听到，无人监听时静默丢弃）
    tx: tokio::sync::broadcast::Sender<()>,
    /// 全扫描互斥：scan 接口并发触发时第二个直接跳过，避免重复 LIST + push
    scanning: AtomicBool,
}

impl PoolSync {
    pub fn new(db: Database, pools: Arc<ResourcePool>, local: Arc<LocalPool>) -> Arc<Self> {
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        Arc::new(Self {
            db,
            pools,
            local,
            states: DashMap::new(),
            known: DashMap::new(),
            tx,
            scanning: AtomicBool::new(false),
        })
    }

    /// 启动后台引擎：先订阅变动事件再全扫描——扫描期间的变动先积压，扫描完随事件
    /// 立即对账，不会出现"扫描后、订阅前"的窗口丢事件。
    pub fn spawn(self: &Arc<Self>) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut rx = this.tx.subscribe();
            let _ = this.full_scan().await;
            loop {
                if rx.recv().await.is_ok() {
                    this.reconcile().await;
                }
            }
        });
    }

    /// 登记一次变动（上传 / 入池 / 出池 / 删除之后调用，不等待对账完成）。
    pub fn notify(&self) {
        let _ = self.tx.send(());
    }

    /// 本服务写池成功后乐观记入实有集，避免增量对账全量重扫。
    pub fn record_write(&self, pool_id: &str, sha256: &str) {
        self.known
            .entry(pool_id.to_string())
            .or_default()
            .insert(sha256.to_string());
    }

    pub fn status(&self, pool_id: &str) -> Option<PoolSyncInfo> {
        self.states.get(pool_id).map(|entry| entry.value().clone())
    }

    pub fn deregister(&self, pool_id: &str) {
        self.states.remove(pool_id);
        self.known.remove(pool_id);
    }

    /// 全扫描互斥：进行中的扫描被并发触发时直接跳过本轮（调用方轮询状态即可）。
    pub async fn full_scan(
        &self,
    ) -> Result<HashMap<String, PoolSyncInfo>, database::error::DatabaseError> {
        if self.scanning.swap(true, Ordering::SeqCst) {
            tracing::warn!("pool scan already in progress, skipping concurrent scan");
            return Ok(HashMap::new());
        }
        let result = self.full_scan_inner().await;
        self.scanning.store(false, Ordering::SeqCst);
        result
    }

    async fn full_scan_inner(
        &self,
    ) -> Result<HashMap<String, PoolSyncInfo>, database::error::DatabaseError> {
        let expected = repo::resource::shas_by_pool(&self.db).await?;
        let mut result = HashMap::new();

        for id in self.pools.ids() {
            if self.pools.get(&id).is_none() {
                continue;
            }

            let scanned_at = Utc::now();
            match self.scan_objects(&id).await {
                Ok(actual) => {
                    tracing::info!(pool = %id, count = actual.len(), "pool scanned");
                    self.known.insert(id.clone(), actual.clone());
                    self.push_missing(&id, expected.get(&id), actual).await;
                    let info = PoolSyncInfo::new(PoolSyncStatus::Synced, 0, Some(scanned_at));
                    self.states.insert(id.clone(), info.clone());
                    result.insert(id, info);
                }
                Err(e) => {
                    tracing::error!(pool = %id, error = %e, "pool scan failed");
                    let info = PoolSyncInfo {
                        status: PoolSyncStatus::Error,
                        pending: 0,
                        scanned_at: Some(scanned_at),
                        error: Some(e.to_string()),
                    };
                    self.states.insert(id.clone(), info.clone());
                    result.insert(id, info);
                }
            }
        }

        Ok(result)
    }

    /// 增量对账：读当前期望，与已知实有对比；尚未建立实有集的池先扫描一次。
    /// DB 读失败静默跳过本轮，等下一变动事件重试（错误在此丢弃，故不在此记日志——由调用方转发来源）。
    async fn reconcile(&self) {
        let Ok(expected) = repo::resource::shas_by_pool(&self.db).await else {
            return;
        };

        for (pool_id, wanted) in expected {
            if self.known.get(&pool_id).is_none() {
                // 新注册（如 reload）或重启后才出现的池：先建立实有集
                match self.scan_objects(&pool_id).await {
                    Ok(actual) => {
                        self.known.insert(pool_id.clone(), actual.clone());
                        self.push_missing(&pool_id, Some(&wanted), actual).await;
                    }
                    Err(e) => tracing::warn!(pool = %pool_id, error = %e, "reconcile scan failed"),
                }
                continue;
            }
            let actual = self
                .known
                .get(&pool_id)
                .map(|entry| entry.value().clone())
                .unwrap_or_default();
            self.push_missing(&pool_id, Some(&wanted), actual).await;
        }
    }

    /// 补齐缺失（模型见模块头）。
    async fn push_missing(
        &self,
        pool_id: &str,
        wanted: Option<&HashSet<String>>,
        actual: HashSet<String>,
    ) {
        let Some(pool) = self.pools.get(pool_id) else {
            return;
        };
        let wanted = wanted.cloned().unwrap_or_default();

        let mut missing: Vec<String> = wanted.difference(&actual).cloned().collect();
        missing.sort();
        if missing.is_empty() {
            return;
        }

        self.states.insert(
            pool_id.to_string(),
            PoolSyncInfo::new(PoolSyncStatus::Syncing, missing.len(), None),
        );

        let mut pushed = 0usize;
        for sha256 in &missing {
            let Ok(key) = object_key(sha256) else {
                continue;
            };
            let mut reader = match self.local.download_file(&key).await {
                Ok(reader) => reader,
                Err(e) => {
                    tracing::warn!(pool = %pool_id, sha256, error = %e, "local copy missing, skip push");
                    continue;
                }
            };
            match pool.upload_file(&key, &mut reader).await {
                Ok(_) => {
                    tracing::info!(pool = %pool_id, sha256 = %sha256, "pushed missing object");
                    pushed += 1;
                    self.known
                        .entry(pool_id.to_string())
                        .or_default()
                        .insert(sha256.clone());
                }
                Err(e) => {
                    tracing::error!(pool = %pool_id, sha256 = %sha256, error = %e, "push failed");
                }
            }
        }

        let remaining = missing.len().saturating_sub(pushed);
        if remaining == 0 {
            self.states.insert(
                pool_id.to_string(),
                PoolSyncInfo::new(PoolSyncStatus::Synced, 0, None),
            );
        } else {
            self.states.insert(
                pool_id.to_string(),
                PoolSyncInfo::new(PoolSyncStatus::Syncing, remaining, None),
            );
        }
    }

    /// 递归（两级分片）列出池内对象，key → 64 位 hex sha256。
    async fn scan_objects(&self, pool_id: &str) -> Result<HashSet<String>, String> {
        let pool = self
            .pools
            .get(pool_id)
            .ok_or_else(|| "pool not connected".to_string())?;

        let mut found = HashSet::new();
        let root = pool.list_dir("").await.map_err(|e| e.to_string())?;

        for entry in root {
            // 一级分片目录：两字符十六进制
            if entry.is_dir
                && entry.name.len() == 2
                && entry.name.bytes().all(|b| b.is_ascii_hexdigit())
            {
                let files = pool
                    .list_dir(&entry.path)
                    .await
                    .map_err(|e| e.to_string())?;
                for file in files {
                    if !file.is_dir {
                        collect_sha(&mut found, &file.name);
                    }
                }
            } else if !entry.is_dir {
                collect_sha(&mut found, &entry.name);
            }
        }
        Ok(found)
    }
}

/// 对象名 → sha256：去掉分片斜杠后必须是 64 位十六进制。
fn collect_sha(found: &mut HashSet<String>, name: &str) {
    let sha256 = name.replace('/', "");
    if sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        found.insert(sha256);
    }
}
