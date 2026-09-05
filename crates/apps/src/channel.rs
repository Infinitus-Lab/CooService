//! 发版通道与差分。

use uuid::Uuid;

/// 一个发版通道。
#[derive(Debug, Clone)]
pub struct Channel {
    pub guid: Uuid,
    pub tag_name: String,
    pub latest_version: String,
    /// 完整包（raw）资源的 sha256
    pub raw_sha256: String,
    pub raw_size: u64,
    /// 客户端优先选这个；没有则取列表第一个
    pub is_default: bool,
}

/// 一条差分：从 `base_sha256` 升到本通道最新版本所需的补丁。
#[derive(Debug, Clone)]
pub struct Diff {
    pub base_sha256: String,
    pub patch_sha256: String,
    pub algo: Option<String>,
    pub size: u64,
}

/// 客户端更新信息：目标版本 + raw 包 + 可用差分。
#[derive(Debug, Clone)]
pub struct Update {
    pub channel: Channel,
    pub diffs: Vec<Diff>,
}
