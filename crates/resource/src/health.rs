//! 资源池健康检查：周期探测远端存储是否可回应，结果共享给上层展示。
//!
//! 本模块只做存储抽象内的存活探测（`RemotePool::list_dir`），不碰数据库；
//! 调度与展示由上层（router）负责，这里只提供监督任务与状态表。
//! `connected`（建连成功）与 `up`（探测通过）是两层：连上了但探测失败 = down。

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use dashmap::{DashMap, DashSet};
use tokio::task::JoinHandle;

use crate::pool::ResourcePool;

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub up: bool,
    pub checked_at: DateTime<Utc>,
    pub error: Option<String>,
}

/// 健康监督者：启动后周期性探测注册表里的每个池。
pub struct PoolHealth {
    interval: Duration,
    timeout: Duration,
    table: DashMap<String, HealthStatus>,
    /// 已进入轮换的池，新注册的池会立即探测一次，避免等满一个周期
    seen: DashSet<String>,
}

impl PoolHealth {
    pub fn new(interval: Duration, timeout: Duration) -> Self {
        Self {
            interval,
            timeout,
            table: DashMap::new(),
            seen: DashSet::new(),
        }
    }

    /// 启动监督任务：按注册表逐池探测，新注册池立即探测（注册语义见 `register`）。
    pub fn start(self: &Arc<Self>, pools: Arc<ResourcePool>) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                let ids: Vec<String> = pools.ids();

                for id in &ids {
                    if !this.seen.contains(id) && this.table.get(id).is_none() {
                        this.probe(&pools, id).await;
                        this.seen.insert(id.clone());
                    }
                }
                for id in &ids {
                    this.probe(&pools, id).await;
                }

                let active: Vec<String> = this.seen.iter().map(|entry| entry.clone()).collect();
                for id in active {
                    if !ids.contains(&id) {
                        this.seen.remove(&id);
                        this.table.remove(&id);
                    }
                }

                tokio::time::sleep(this.interval).await;
            }
        })
    }

    pub fn status(&self, id: &str) -> Option<HealthStatus> {
        self.table.get(id).map(|entry| entry.value().clone())
    }

    /// 池被删除后清掉状态，避免探测与展示残留。
    pub fn deregister(&self, id: &str) {
        self.seen.remove(id);
        self.table.remove(id);
    }

    async fn probe(&self, pools: &ResourcePool, id: &str) {
        let Some(pool) = pools.get(id) else {
            return;
        };

        let status = match tokio::time::timeout(self.timeout, pool.list_dir("")).await {
            Ok(Ok(_)) => HealthStatus {
                up: true,
                checked_at: Utc::now(),
                error: None,
            },
            Ok(Err(e)) => HealthStatus {
                up: false,
                checked_at: Utc::now(),
                error: Some(format!("{e}")),
            },
            Err(_) => HealthStatus {
                up: false,
                checked_at: Utc::now(),
                error: Some(format!("probe timeout after {:?}", self.timeout)),
            },
        };

        self.table.insert(id.to_string(), status);
    }
}
