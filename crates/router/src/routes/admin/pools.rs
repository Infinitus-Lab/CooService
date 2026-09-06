//! `/api/v1/admin/pools`：资源池配置的增删改查，落在 `resource_pool` 表。
//!
//! 只存配置（终结点 / 密钥 / kind 相关参数），真正建连由 `resource` crate 做。
//! 在线情况 = 运行时注册表（connected）+ 健康探测（up，resource::health 维护）；
//! 同步情况 = 池同步引擎（期望登记 vs 实有，`pool_sync` 维护）。
//! `secret` 只在写请求里出现，任何响应都不回显。

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use database::repo;
use resource::pool::RemotePool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ApiOk, ApiResult, AppError, response::with_status, state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/scan", post(scan))
        .route("/{id}", get(detail).patch(modify).delete(remove))
        .route(
            "/{id}/resources",
            get(pool_resources).post(pool_add_resource),
        )
        .route("/{id}/resources/{sha256}", delete(pool_remove_resource))
}

#[derive(Debug, Serialize)]
pub struct PoolView {
    pub id: String,
    pub kind: String,
    pub endpoint: String,
    pub public_endpoint: String,
    /// kind 相关参数：S3 是 bucket/region，FTP 是 root
    pub config: Value,
    /// 是否参与公开下载 302 重定向
    pub is_public: bool,
    /// 运行时注册表里有没有连接（建连成功）
    pub connected: bool,
    /// 健康探测是否通过（resource::health 每周期 list_dir 探测）
    pub up: bool,
    /// 最近一次健康探测时间
    pub health_checked_at: Option<DateTime<Utc>>,
    pub health_error: Option<String>,
    /// syncing / synced / error
    pub sync_status: String,
    /// 待补齐的推送操作数
    pub sync_pending: usize,
    pub sync_scanned_at: Option<DateTime<Utc>>,
    pub sync_error: Option<String>,
    /// 登记量统计（resource_location）
    pub object_count: i64,
    pub total_size: i64,
    pub last_write_at: Option<DateTime<Utc>>,
}

impl PoolView {
    /// 组装完整视图：行数据 + 运行时状态 + 登记统计。
    fn compose(
        state: &AppState,
        row: repo::pool::PoolRow,
        stats: &HashMap<String, repo::pool::PoolStats>,
    ) -> Self {
        let id = row.id.clone();
        let health = state.health.status(&id);
        let sync = state.sync.status(&id);
        let stat = stats.get(&id);

        let connected = state.resources.get(&id).is_some();

        Self {
            id: row.id,
            kind: row.kind,
            endpoint: row.endpoint,
            public_endpoint: row.public_endpoint,
            config: row.config,
            is_public: row.is_public,
            connected,
            up: connected && health.as_ref().is_some_and(|h| h.up),
            health_checked_at: health.as_ref().map(|h| h.checked_at),
            health_error: health.and_then(|h| h.error),
            sync_status: sync
                .as_ref()
                .map(|s| s.status.as_str().to_string())
                .unwrap_or_else(|| "unknown".into()),
            sync_pending: sync.as_ref().map(|s| s.pending).unwrap_or(0),
            sync_scanned_at: sync.as_ref().and_then(|s| s.scanned_at),
            sync_error: sync.and_then(|s| s.error),
            object_count: stat.map(|s| s.object_count).unwrap_or(0),
            total_size: stat.map(|s| s.total_size).unwrap_or(0),
            last_write_at: stat.and_then(|s| s.last_write_at),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePoolRequest {
    pub id: String,
    /// `s3` 或 `ftp`
    pub kind: String,
    pub endpoint: String,
    pub public_endpoint: String,
    pub secret: String,
    #[serde(default)]
    pub config: Option<Value>,
    /// 置 true 要求 public_endpoint 非空（schema 层 CHECK 亦约束）
    #[serde(default)]
    pub is_public: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePoolRequest {
    pub endpoint: Option<String>,
    pub public_endpoint: Option<String>,
    pub secret: Option<String>,
    pub config: Option<Value>,
    pub is_public: Option<bool>,
}

async fn list(State(state): State<AppState>) -> ApiResult<Vec<PoolView>> {
    let stats = repo::pool::stats(&state.db).await?;
    let rows = repo::pool::list(&state.db).await?;

    Ok(ApiOk(
        rows.into_iter()
            .map(|row| PoolView::compose(&state, row, &stats))
            .collect(),
    ))
}

async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreatePoolRequest>,
) -> Result<Response, AppError> {
    validate_kind(&body.kind)?;

    let config = body
        .config
        .unwrap_or_else(|| Value::Object(Default::default()));
    validate_config(&body.kind, &config)?;

    validate_public_endpoint(&body.public_endpoint, body.is_public)?;

    // 凭据加密落库（见 `resource::secret`）；未配置主密钥时明文直存并告警
    let secret = resource::secret::encrypt_secret(&body.secret);

    repo::pool::insert(
        &state.db,
        &repo::pool::NewPool {
            id: &body.id,
            kind: &body.kind,
            endpoint: &body.endpoint,
            public_endpoint: &body.public_endpoint,
            secret: &secret,
            config: &config,
            is_public: body.is_public,
        },
    )
    .await?;

    let view = PoolView {
        id: body.id,
        kind: body.kind,
        endpoint: body.endpoint,
        public_endpoint: body.public_endpoint,
        config,
        is_public: body.is_public,
        connected: false,
        up: false,
        health_checked_at: None,
        health_error: None,
        sync_status: "unknown".into(),
        sync_pending: 0,
        sync_scanned_at: None,
        sync_error: None,
        object_count: 0,
        total_size: 0,
        last_write_at: None,
    };
    Ok(with_status(StatusCode::CREATED, view))
}

async fn detail(Path(id): Path<String>, State(state): State<AppState>) -> ApiResult<PoolView> {
    let stats = repo::pool::stats(&state.db).await?;
    let row = repo::pool::get(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(id))?;

    Ok(ApiOk(PoolView::compose(&state, row, &stats)))
}

async fn modify(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<UpdatePoolRequest>,
) -> ApiResult<PoolView> {
    let row = repo::pool::get(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(id.clone()))?;

    // 改过 config 就先按现有 kind 校验，挡在落库之前
    if let Some(config) = body.config.as_ref() {
        validate_config(&row.kind, config)?;
    }

    // 生效后的公开终结点与开关（含未改动的现值），统一校验 scheme 与组合约束
    let effective_public = body.public_endpoint.as_deref().unwrap_or(&row.public_endpoint);
    let effective_is_public = body.is_public.unwrap_or(row.is_public);
    validate_public_endpoint(effective_public, effective_is_public)?;

    // 凭据加密落库（`resource::secret`）；未配置主密钥时明文直存并告警
    let secret = body.secret.as_deref().map(resource::secret::encrypt_secret);

    let updated = repo::pool::update(
        &state.db,
        &repo::pool::PoolUpdate {
            id: &id,
            endpoint: body.endpoint.as_deref(),
            public_endpoint: body.public_endpoint.as_deref(),
            secret: secret.as_deref(),
            config: body.config.as_ref(),
            is_public: body.is_public,
        },
    )
    .await?;
    if !updated {
        return Err(AppError::NotFound(id));
    }

    // 运行时公开信息就地同步（is_public / public_endpoint 变化立刻生效，无需 reload）
    let row = repo::pool::get(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    state.pool_meta.insert(
        id.clone(),
        crate::state::PoolMeta {
            public_endpoint: row.public_endpoint.clone(),
            is_public: row.is_public,
        },
    );

    let stats = repo::pool::stats(&state.db).await?;
    Ok(ApiOk(PoolView::compose(&state, row, &stats)))
}

async fn remove(Path(id): Path<String>, State(state): State<AppState>) -> ApiResult<String> {
    if !repo::pool::delete(&state.db, &id).await? {
        return Err(AppError::NotFound(id));
    }

    // 库行删掉后清运行时残留：注册表、公开信息、健康探测、同步状态
    state.resources.remove(&id);
    state.pool_meta.remove(&id);
    state.health.deregister(&id);
    state.sync.deregister(&id);

    Ok(ApiOk(id))
}

/// 触发后台全扫描并立即返回：扫描可能远超请求超时窗口（池大时逐池 LIST + 推补缺失），
/// 同步引擎会把结果写回各池状态，管理界面轮询 pools 列表即可看到进度。
async fn scan(State(state): State<AppState>) -> Result<Response, AppError> {
    let sync = Arc::clone(&state.sync);
    tokio::spawn(async move {
        if let Err(e) = sync.full_scan().await {
            tracing::error!(error = %e, "background pool scan failed");
        }
    });

    let view: HashMap<String, String> = state
        .resources
        .ids()
        .into_iter()
        .map(|id| {
            let status = state
                .sync
                .status(&id)
                .map(|s| s.status.as_str().to_string())
                .unwrap_or_else(|| "unknown".into());
            (id, status)
        })
        .collect();
    Ok(with_status(StatusCode::ACCEPTED, view))
}

// ---------- 池成员管理（资源进池/出池是资源池的领域） ----------

#[derive(Debug, Serialize)]
pub struct PoolResourceView {
    pub sha256: String,
    pub name: Option<String>,
    pub size: i64,
    pub added_at: DateTime<Utc>,
}

async fn pool_resources(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Vec<PoolResourceView>> {
    if repo::pool::get(&state.db, &id).await?.is_none() {
        return Err(AppError::NotFound(id));
    }

    let rows = repo::resource::list_by_pool(&state.db, &id).await?;
    Ok(ApiOk(
        rows.into_iter()
            .map(|row| PoolResourceView {
                sha256: row.sha256,
                name: row.name,
                size: row.size,
                added_at: row.added_at,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct AddResourceRequest {
    pub sha256: String,
}

/// 把本地资源放进池：本地副本为源推到池 → 登记 location（= 同步引擎的期望 +1）。
async fn pool_add_resource(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<AddResourceRequest>,
) -> Result<Response, AppError> {
    if repo::pool::get(&state.db, &id).await?.is_none() {
        return Err(AppError::NotFound(id));
    }
    let sha256 = resource::key::normalize_sha256(&body.sha256)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let resource = repo::resource::get(&state.db, &sha256)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("resource {sha256} does not exist")))?;
    if resource.pools.iter().any(|p| p == &id) {
        return Err(AppError::Conflict(format!(
            "resource {sha256} already in pool {id}"
        )));
    }

    let pool = state
        .resources
        .get(&id)
        .ok_or_else(|| AppError::BadRequest(format!("pool {id:?} not connected")))?;

    let key = resource::key::object_key(&sha256).expect("normalized above");
    let mut reader = state
        .local
        .download_file(&key)
        .await
        .map_err(|e| AppError::BadRequest(format!("local copy missing, upload first: {e}")))?;
    if let Err(e) = pool.upload_file(&key, &mut reader).await {
        return Err(AppError::BadGateway(format!(
            "push to pool {id:?} failed: {e}"
        )));
    }

    repo::resource::add_location(&state.db, &sha256, &id).await?;
    state.sync.record_write(&id, &sha256);
    state.sync.notify();

    let row = repo::resource::list_by_pool(&state.db, &id)
        .await?
        .into_iter()
        .find(|r| r.sha256 == sha256)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("location just inserted")))?;

    Ok(with_status(
        StatusCode::CREATED,
        PoolResourceView {
            sha256: row.sha256,
            name: row.name,
            size: row.size,
            added_at: row.added_at,
        },
    ))
}

/// 从池里移除：先注销 location（期望集不再要求该对象），再删池内对象。
/// 顺序不能反：若先删对象、后注销登记失败，期望集仍含该 sha、实有集仍记
/// 存在，同步引擎不会补推 → 对象在池内静默丢失。反向顺序失败只留孤儿文件。
async fn pool_remove_resource(
    Path((id, sha256)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<String> {
    let sha256 = resource::key::normalize_sha256(&sha256)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    if !repo::resource::remove_location(&state.db, &sha256, &id).await? {
        return Err(AppError::NotFound(format!(
            "resource {sha256} not in pool {id}"
        )));
    }

    let key = resource::key::object_key(&sha256).expect("normalized above");
    if let Some(pool) = state.resources.get(&id)
        && let Err(e) = pool.delete_file(&key).await
    {
        tracing::warn!(pool = %id, sha256, error = %e, "delete pool object failed, orphan left");
    }
    state.sync.notify();

    Ok(ApiOk(sha256))
}

fn validate_kind(kind: &str) -> Result<(), AppError> {
    match kind {
        "s3" | "ftp" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "invalid pool kind {other:?}, expected s3 or ftp"
        ))),
    }
}

/// 公开终结点必须是 http/https URL；公开池（is_public）必须填（DB CHECK 同规则，这里提前 400）。
fn validate_public_endpoint(endpoint: &str, is_public: bool) -> Result<(), AppError> {
    if endpoint.is_empty() {
        return if is_public {
            Err(AppError::BadRequest(
                "is_public requires a non-empty public_endpoint".into(),
            ))
        } else {
            Ok(())
        };
    }
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err(AppError::BadRequest(
            "public_endpoint must start with http:// or https://".into(),
        ));
    }
    Ok(())
}

/// 按 kind 校验 config 必填字段：s3 必须给 bucket/region/access_key（secret 是 secret key），
/// ftp 必须给 user（secret 是密码）。缺失直接 400，别等 reload 建连时才暴露。
fn validate_config(kind: &str, config: &Value) -> Result<(), AppError> {
    if !config.is_object() {
        return Err(AppError::BadRequest("config must be a JSON object".into()));
    }

    let require_str = |key: &str| -> Result<(), AppError> {
        if config
            .get(key)
            .and_then(|v| v.as_str())
            .is_none_or(|s| s.is_empty())
        {
            return Err(AppError::BadRequest(format!(
                "{kind} pool requires config.{key}"
            )));
        }
        Ok(())
    };

    match kind {
        "s3" => {
            for key in ["bucket", "region", "access_key"] {
                require_str(key)?;
            }
            if let Some(path_style) = config.get("path_style")
                && !path_style.is_boolean()
            {
                return Err(AppError::BadRequest(
                    "s3 pool config.path_style must be a boolean".into(),
                ));
            }
        }
        "ftp" => {
            require_str("user")?;
            if let Some(port) = config.get("port")
                && !port.is_number()
            {
                return Err(AppError::BadRequest(
                    "ftp pool config.port must be a number".into(),
                ));
            }
        }
        _ => {}
    }

    Ok(())
}
