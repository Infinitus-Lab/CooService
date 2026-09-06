//! HTTP 路由层。
//!
//! 分层：`config` 读环境变量 → `state` 组装共享状态 → `routes` 挂 handler
//! → `router` 装配路由树与中间件；错误与响应分别走 `error` / `response`。

pub mod auth;
pub mod cache;
pub mod config;
pub mod error;
pub mod extract;
pub mod pool_sync;
pub mod pools;
pub mod ratelimit;
pub mod response;
pub mod router;
pub mod routes;
pub mod state;

pub use config::ServerConfig;
pub use error::{ApiResult, AppError};
pub use response::{ApiOk, Envelope};
pub use router::build_router;
pub use state::AppState;