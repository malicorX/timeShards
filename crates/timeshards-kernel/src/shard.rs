use crate::{EventBus, ShardManifest};
use async_trait::async_trait;
use std::sync::Arc;
use timeshards_core::DomainEvent;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShardId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardHealth {
    Stopped,
    Starting,
    Running,
    Degraded,
}

pub struct KernelContext {
    pub bus: EventBus,
}

#[async_trait]
pub trait Shard: Send + Sync {
    fn manifest(&self) -> &ShardManifest;
    async fn start(&mut self, ctx: &KernelContext) -> anyhow::Result<()>;
    async fn stop(&mut self) -> anyhow::Result<()>;
    async fn on_event(&self, event: Arc<DomainEvent>) -> anyhow::Result<()> {
        let _ = event;
        Ok(())
    }
    fn health(&self) -> ShardHealth;
}
