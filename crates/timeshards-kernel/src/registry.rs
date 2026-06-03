use crate::{EventBus, KernelContext, Shard, ShardHealth, ShardId};
use std::collections::HashMap;
use tracing::info;

pub struct ShardRegistry {
    bus: EventBus,
    shards: HashMap<ShardId, Box<dyn Shard>>,
}

impl Default for ShardRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ShardRegistry {
    pub fn new() -> Self {
        Self {
            bus: EventBus::new(),
            shards: HashMap::new(),
        }
    }

    pub fn event_bus(&self) -> EventBus {
        self.bus.clone()
    }

    pub async fn register(&mut self, mut shard: Box<dyn Shard>) -> anyhow::Result<()> {
        let id = ShardId(shard.manifest().id.clone());
        info!(shard = %id.0, "registering shard");
        let ctx = KernelContext {
            bus: self.bus.clone(),
        };
        shard.start(&ctx).await?;
        self.shards.insert(id, shard);
        Ok(())
    }

    pub async fn stop_all(&mut self) -> anyhow::Result<()> {
        for (id, shard) in self.shards.iter_mut() {
            info!(shard = %id.0, "stopping shard");
            shard.stop().await?;
        }
        Ok(())
    }

    pub fn health_report(&self) -> Vec<(String, ShardHealth)> {
        self.shards
            .iter()
            .map(|(id, s)| (id.0.clone(), s.health()))
            .collect()
    }
}
