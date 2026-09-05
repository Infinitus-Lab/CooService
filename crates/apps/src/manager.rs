//! 应用管理器：`app` 表是唯一真相，内存只是一份缓存。

use dashmap::DashMap;
use database::{
    Database,
    repo::{app, channel},
};
use uuid::Uuid;

use crate::{
    channel::{Channel, Diff, Update},
    error::AppError,
    instance::AppInstance,
};

pub struct AppManager {
    db: Database,
    apps: DashMap<Uuid, AppInstance>,
}

impl AppManager {
    /// 从 `app` 表载入所有启用的 app。
    pub async fn load(db: Database) -> Result<Self, AppError> {
        let manager = Self {
            db,
            apps: DashMap::new(),
        };
        manager.reload().await?;
        Ok(manager)
    }

    /// 重读 `app` 表：先清内存再灌入，禁用或删掉的 app 会随之消失。
    pub async fn reload(&self) -> Result<(), AppError> {
        let rows = app::list_enabled(&self.db).await?;

        self.apps.clear();
        for row in rows {
            self.apps.insert(row.id, AppInstance::new(row.id, row.name));
        }

        tracing::info!(count = self.apps.len(), "apps loaded");
        Ok(())
    }

    /// 管理接口用：含禁用的，全部从库里读。
    pub async fn list_all(&self) -> Result<Vec<AppInstance>, AppError> {
        let rows = app::list_all(&self.db).await?;
        Ok(rows
            .into_iter()
            .map(|row| AppInstance::new(row.id, row.name))
            .collect())
    }

    pub fn get(&self, id: Uuid) -> Option<AppInstance> {
        self.apps.get(&id).map(|app| app.value().clone())
    }

    /// 改 name 和/或 enabled，传 `None` 的字段不动。返回改后的实例。
    pub async fn set_info(
        &self,
        id: Uuid,
        name: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<AppInstance, AppError> {
        if !app::update(&self.db, id, name, enabled).await? {
            return Err(AppError::NotFound(id.to_string()));
        }

        let row = app::get(&self.db, id)
            .await?
            .ok_or_else(|| AppError::NotFound(id.to_string()))?;

        let app = AppInstance::new(row.id, row.name);
        if row.enabled {
            self.apps.insert(row.id, app.clone());
        } else {
            self.apps.remove(&row.id);
        }
        Ok(app)
    }

    pub fn list(&self) -> Vec<AppInstance> {
        self.apps.iter().map(|app| app.value().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.apps.len()
    }

    /// 在 `app` 表登记并载入内存；重复返回 `AlreadyExists`。
    pub async fn create(&self, id: Uuid, name: &str) -> Result<AppInstance, AppError> {
        if app::exists(&self.db, id).await? {
            return Err(AppError::AlreadyExists(id.to_string()));
        }

        app::insert(&self.db, id, name).await?;
        let app = AppInstance::new(id, name.to_string());
        self.apps.insert(id, app.clone());
        Ok(app)
    }

    /// 只改 `enabled` 一列，行本身保留；禁用的 app 从内存移除。
    pub async fn set_enable(&self, id: Uuid, enable: bool) -> Result<(), AppError> {
        if !app::set_enabled(&self.db, id, enable).await? {
            return Err(AppError::NotFound(id.to_string()));
        }

        match enable {
            true => {
                if let Some(row) = app::get(&self.db, id).await? {
                    self.apps.insert(row.id, AppInstance::new(row.id, row.name));
                }
            }
            false => {
                self.apps.remove(&id);
            }
        }

        Ok(())
    }

    /// 从 `app` 表删掉整行。
    pub async fn remove(&self, id: Uuid) -> Result<AppInstance, AppError> {
        let row = app::get(&self.db, id)
            .await?
            .ok_or_else(|| AppError::NotFound(id.to_string()))?;
        app::delete(&self.db, id).await?;

        let app = AppInstance::new(row.id, row.name);
        self.apps.remove(&id);
        Ok(app)
    }

    /// 列出 app 的通道，默认通道排最前。app 不存在或已禁用返回 `NotFound`。
    pub async fn channels(&self, id: Uuid) -> Result<Vec<Channel>, AppError> {
        // 与公告接口一致：禁用 app 已从内存移除，视为不存在
        if self.apps.get(&id).is_none() {
            return Err(AppError::NotFound(id.to_string()));
        }

        let rows = channel::list_by_app(&self.db, id).await?;
        Ok(rows.into_iter().map(Channel::from_row).collect())
    }

    /// 按 channel guid 取更新信息；guid 不存在返回 `Ok(None)`。
    pub async fn update(&self, guid: Uuid) -> Result<Option<Update>, AppError> {
        let row = match channel::get(&self.db, guid).await? {
            Some(row) => row,
            None => return Ok(None),
        };
        let diffs = channel::list_diffs(&self.db, guid).await?;

        Ok(Some(Update {
            channel: Channel::from_row(row),
            diffs: diffs
                .into_iter()
                .map(|row| Diff {
                    base_sha256: row.base_sha256,
                    patch_sha256: row.patch_sha256,
                    algo: row.algo,
                    size: row.size as u64,
                })
                .collect(),
        }))
    }
}

impl Channel {
    fn from_row(row: database::repo::channel::ChannelRow) -> Self {
        Self {
            guid: row.guid,
            tag_name: row.tag_name,
            latest_version: row.latest_version,
            raw_sha256: row.raw_sha256,
            raw_size: row.raw_size as u64,
            is_default: row.is_default,
        }
    }
}
