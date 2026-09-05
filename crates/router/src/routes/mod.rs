//! 子路由树与 handler。
//!
//! 公开接口在 `app`（客户端）与 `resources`（下载），
//! 管理接口全部在 `admin/` 目录下，由 `admin/mod.rs` 的 `X-Admin-Key` 中间件统一保护。
//! 两类接口严格分文件，不混写。

pub mod admin;
pub mod app;
pub mod health;
pub mod resources;
pub mod v1;
