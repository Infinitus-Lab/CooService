//! 服务入口。

use std::sync::Arc;

use apps::manager::AppManager;
use database::{Database, DatabaseConfig};
use resource::local::LocalPool;
use router::{
    AppState, ServerConfig, auth::generate_admin_key, build_router, pools::load_resource_pools,
};

#[tokio::main]
async fn main() {
    // 日志先起来，run() 里的失败才能被记录
    init_tracing();

    if let Err(err) = run().await {
        // {err:#} 展开 anyhow 完整错误链
        tracing::error!("server failed: {err:#}");
        std::process::exit(1);
    }
}

/// 启动即失败上抛；退出码与日志归 `main` 统一处理。
async fn run() -> anyhow::Result<()> {
    let config = ServerConfig::from_env()?;
    let db = Database::connect(&DatabaseConfig::from_env()?).await?;
    let apps = AppManager::load(db.clone()).await?;
    let (resources, pool_meta) = load_resource_pools(&db).await?;
    // 共享句柄统一 Arc：健康探测与同步引擎与路由共用同一注册表
    let resources = Arc::new(resources);
    let local = Arc::new(LocalPool::connect(&config.local_resource_dir).await?);
    // 启动时校验本地副本可写：挂载属主 / SELinux 标签错误应在起步阶段暴露
    let probe = local.root().join(".coo-write-probe");
    tokio::fs::write(&probe, b"").await.map_err(|e| {
        anyhow::anyhow!(
            "local copy dir {:?} is not writable: {e}; check mount ownership (65532) and SELinux label (container_file_t)",
            local.root()
        )
    })?;
    tokio::fs::remove_file(&probe).await.ok();
    // 存活性探测归 resource crate（存储抽象自带），router 只调度与读状态
    let health = Arc::new(resource::health::PoolHealth::new(
        config.pool_health_interval,
        config.pool_health_timeout,
    ));
    health.start(resources.clone());
    // 池同步引擎需读 DB 的期望登记，按期放在 router 层
    let sync = router::pool_sync::PoolSync::new(db.clone(), resources.clone(), local.clone());
    sync.spawn();
    let admin_key = generate_admin_key();
    let state = AppState::new(
        apps,
        resources,
        pool_meta,
        local,
        health,
        sync,
        db,
        admin_key.clone(),
    );
    let app = build_router(state, config.request_timeout);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    // 管理密钥不落盘，只在启动日志里出现一次
    tracing::info!(%admin_key, "admin key (X-Admin-Key)");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("server stopped");
    Ok(())
}

/// 日志级别取自 `RUST_LOG`，未设置时用下面的默认值。
fn init_tracing() {
    let filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=debug,router=debug".into());

    // 容器里全是文件日志，ANSI 只会污染解析
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .init();
}

/// 等 Ctrl-C（Unix 上还有 SIGTERM），任一信号到达即返回。
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("received Ctrl-C, shutting down"),
        () = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}
