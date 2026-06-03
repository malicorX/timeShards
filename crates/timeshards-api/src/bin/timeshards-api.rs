//! Headless API server for local smoke tests and CI (no Tauri UI).
//!
//! Env: `TIMESHARDS_DB` (default: `./.data/timeshards-api.db`),
//!      `TIMESHARDS_API_HOST`, `TIMESHARDS_API_PORT` (default 47821).

use std::path::PathBuf;
use std::sync::Arc;
use timeshards_api::{run_api_server, ApiConfig, AppState};
use timeshards_db::{ensure_demo_accounts, seed_if_empty, sync_role_permissions, Database};
use timeshards_api::spawn_credential_worker;
use timeshards_hardware::bootstrap_hardware;
use timeshards_kernel::ShardRegistry;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let db_path = std::env::var("TIMESHARDS_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".data/timeshards-api.db"));
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Database::connect(&db_path).await?;
    seed_if_empty(&db.pool).await?;
    sync_role_permissions(&db.pool).await?;
    ensure_demo_accounts(&db.pool).await?;

    let hw = bootstrap_hardware().await?;

    let state = Arc::new(AppState::new(
        db.pool.clone(),
        ShardRegistry::new(),
        hw.gateway,
        hw.inject,
    ));

    spawn_credential_worker(state.clone(), hw.events_rx);

    tracing::info!(path = %db_path.display(), "database");
    run_api_server(state, ApiConfig::from_env()).await
}
