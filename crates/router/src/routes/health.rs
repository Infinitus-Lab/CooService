//! `/healthz`

use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{ApiOk, state::AppState};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub started_at: DateTime<Utc>,
}

pub async fn health(State(state): State<AppState>) -> ApiOk<HealthResponse> {
    ApiOk(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        started_at: state.started_at,
    })
}
