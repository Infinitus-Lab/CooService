//! 应用实例。

use uuid::Uuid;

/// 一个应用实例，由 AppId 与 `app.name` 初始化。
/// Channels / Resources / Announces 不在此持有，分别从 `channel`、`app_resource`、`announce` 表按需读取。
#[derive(Debug, Clone)]
pub struct AppInstance {
    pub id: Uuid,
    pub name: String,
}

impl AppInstance {
    pub fn new(id: Uuid, name: String) -> Self {
        Self { id, name }
    }
}
