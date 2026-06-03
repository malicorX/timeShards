use crate::gateway::{HardwareEvent, HardwareGateway};
use crate::tcp_ingest::spawn_tcp_credential_listener;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::config::hardware_tcp_listen_addr;
use super::simulator::HardwareEventSender;

/// External / Primion-style adapter: TCP JSON ingest + manual `inject` / REST `hardware-present`.
pub struct ExternalGateway {
    id: String,
    events: HardwareEventSender,
    tcp_task: Mutex<Option<JoinHandle<()>>>,
}

impl ExternalGateway {
    pub fn new(events: HardwareEventSender) -> Self {
        Self {
            id: "hardware.external".into(),
            events,
            tcp_task: Mutex::new(None),
        }
    }
}

#[async_trait]
impl HardwareGateway for ExternalGateway {
    fn adapter_id(&self) -> &str {
        &self.id
    }

    async fn start(&self) -> anyhow::Result<()> {
        if let Some(addr) = hardware_tcp_listen_addr() {
            let handle = spawn_tcp_credential_listener(addr.clone(), self.events.clone());
            *self.tcp_task.lock().await = Some(handle);
            info!(adapter = %self.id, addr = %addr, "external hardware adapter started");
        } else {
            warn!(
                adapter = %self.id,
                "no TIMESHARDS_HW_TCP_ADDR — use REST simulate-scan, hardware-present, or set TCP listen address"
            );
        }
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        if let Some(handle) = self.tcp_task.lock().await.take() {
            handle.abort();
        }
        Ok(())
    }

    async fn inject(&self, event: HardwareEvent) -> anyhow::Result<()> {
        self.events.send(event)?;
        Ok(())
    }
}
