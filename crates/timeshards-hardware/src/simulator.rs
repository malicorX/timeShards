use crate::gateway::{HardwareEvent, HardwareGateway, RawCredentialPresentation};
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::debug;

pub type HardwareEventSender = mpsc::UnboundedSender<HardwareEvent>;

/// In-process simulator for development without physical readers.
pub struct SimulatorGateway {
    id: String,
    events: Option<HardwareEventSender>,
}

impl SimulatorGateway {
    pub fn new(events: HardwareEventSender) -> Self {
        Self {
            id: "hardware.simulator".into(),
            events: Some(events),
        }
    }

    pub fn detached() -> Self {
        Self {
            id: "hardware.simulator".into(),
            events: None,
        }
    }
}

#[async_trait]
impl HardwareGateway for SimulatorGateway {
    fn adapter_id(&self) -> &str {
        &self.id
    }

    async fn start(&self) -> anyhow::Result<()> {
        debug!(adapter = %self.id, "simulator gateway started");
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn inject(&self, event: HardwareEvent) -> anyhow::Result<()> {
        if let Some(tx) = &self.events {
            tx.send(event)?;
        }
        Ok(())
    }
}

impl SimulatorGateway {
    pub async fn simulate_badge_scan(
        &self,
        reader_id: impl Into<String>,
        credential_uid: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.inject(HardwareEvent::CredentialPresented(RawCredentialPresentation {
            reader_id: reader_id.into(),
            credential_uid: credential_uid.into(),
            occurred_at: Utc::now(),
        }))
        .await
    }
}

pub fn channel() -> (HardwareEventSender, mpsc::UnboundedReceiver<HardwareEvent>) {
    mpsc::unbounded_channel()
}
