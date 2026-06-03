use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Normalized hardware events — all adapters must map to this shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCredentialPresentation {
    pub reader_id: String,
    pub credential_uid: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HardwareEvent {
    CredentialPresented(RawCredentialPresentation),
    DoorStateChanged {
        door_id: Uuid,
        state: String,
        occurred_at: DateTime<Utc>,
    },
    ReaderOffline {
        reader_id: String,
        occurred_at: DateTime<Utc>,
    },
    Heartbeat {
        device_id: String,
        occurred_at: DateTime<Utc>,
    },
}

#[async_trait]
pub trait HardwareGateway: Send + Sync {
    fn adapter_id(&self) -> &str;
    async fn start(&self) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
    /// Inject a simulated event (no-op on real adapters unless in test mode).
    async fn inject(&self, event: HardwareEvent) -> anyhow::Result<()>;
}
