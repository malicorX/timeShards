use axum::Router;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::{routes::api_router, state::AppState};

#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl ApiConfig {
    pub fn from_env() -> Self {
        let host = env::var("TIMESHARDS_API_HOST").unwrap_or_else(|_| "0.0.0.0".into());
        let port = env::var("TIMESHARDS_API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(47821);
        Self { host, port }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// URLs clients can use (localhost + LAN IP when detectable).
    pub fn client_urls(&self) -> Vec<String> {
        let mut urls = vec![format!("http://127.0.0.1:{}", self.port)];
        if let Ok(ip) = local_ip_address::local_ip() {
            if !ip.is_loopback() {
                urls.push(format!("http://{}:{}", ip, self.port));
            }
        }
        urls
    }
}

pub async fn run_api_server(state: Arc<AppState>, config: ApiConfig) -> anyhow::Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new().merge(api_router(state.clone())).layer(cors);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    info!(%addr, urls = ?config.client_urls(), "API server listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
