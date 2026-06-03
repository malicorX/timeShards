use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use timeshards_api::{run_api_server, spawn_credential_worker, ApiConfig, AppState};
use timeshards_db::{ensure_demo_accounts, seed_if_empty, sync_role_permissions, Database};
use timeshards_hardware::bootstrap_hardware;
use timeshards_kernel::ShardRegistry;
use tracing_subscriber::EnvFilter;

#[derive(Clone, serde::Serialize)]
pub struct ServerInfo {
    pub api_bind: String,
    pub api_urls: Vec<String>,
    pub database_path: String,
}

struct ServerRuntime {
    api_bind: String,
    api_urls: Vec<String>,
    database_path: String,
}

fn data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir")
        .join("TimeShards")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = data_dir(&handle);
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("server.db");
            let config = ApiConfig::from_env();
            let api_bind = format!("{}:{}", config.host, config.port);
            let api_urls = config.client_urls();

            app.manage(ServerRuntime {
                api_bind: api_bind.clone(),
                api_urls: api_urls.clone(),
                database_path: db_path.display().to_string(),
            });

            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_backend(handle, db_path, config).await {
                    tracing::error!(error = %e, "server backend failed");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_server_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn start_backend(
    _app: tauri::AppHandle,
    db_path: PathBuf,
    config: ApiConfig,
) -> anyhow::Result<()> {
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

    run_api_server(state, config).await
}

#[tauri::command]
fn get_server_info(app: tauri::AppHandle) -> Result<ServerInfo, String> {
    let rt = app
        .try_state::<ServerRuntime>()
        .ok_or("Server noch nicht gestartet")?;
    Ok(ServerInfo {
        api_bind: rt.api_bind.clone(),
        api_urls: rt.api_urls.clone(),
        database_path: rt.database_path.clone(),
    })
}
