//! `/healthz`：探活只回状态与启动时间，不回版本号（版本信息利于指纹识别，无业务价值）。

use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{ApiOk, state::AppState};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub started_at: DateTime<Utc>,
}

pub async fn health(State(state): State<AppState>) -> ApiOk<HealthResponse> {
    ApiOk(HealthResponse {
        status: "ok",
        started_at: state.started_at,
    })
}
