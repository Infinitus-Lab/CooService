//! 公开端点的短 TTL 缓存。
//!
//! 客户端接口无认证、变化频率低（通道 / 公告都是人工维护），每次请求打 DB 是浪费；
//! 缓存 TTL 几秒级，命中即省一次跨表聚合查询。

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use serde_json::Value;

#[derive(Clone)]
pub struct PublicCache {
    ttl: Duration,
    entries: Arc<DashMap<String, (Instant, Value)>>,
}

impl PublicCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Arc::new(DashMap::new()),
        }
    }

    /// 命中且未过期则返回缓存值，否则 `None`（过期条目顺手删掉）。
    pub fn get(&self, key: &str) -> Option<Value> {
        let entry = self.entries.get(key)?;
        let (expired, value) = {
            let e = entry.value();
            (e.0.elapsed() >= self.ttl, e.1.clone())
        };
        // 先归还读锁再删，否则同一 shard 上读写锁自锁
        drop(entry);
        if expired {
            self.entries.remove(key);
            return None;
        }
        Some(value)
    }

    pub fn insert(&self, key: String, value: Value) {
        // 放不下就清掉最旧的一个，缓存只是优化不是语义
        if self.entries.len() >= 1024
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|e| e.value().0)
                .map(|e| e.key().clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, (Instant::now(), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_expiry() {
        let cache = PublicCache::new(Duration::from_secs(10));
        cache.insert("k".into(), Value::Null);
        assert!(cache.get("k").is_some());
        cache
            .entries
            .get_mut(&"k".to_string())
            .unwrap()
            .0 = Instant::now() - Duration::from_secs(11);
        assert!(cache.get("k").is_none());
    }
}