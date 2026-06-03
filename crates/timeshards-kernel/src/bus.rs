use std::sync::Arc;
use timeshards_core::DomainEvent;
use tokio::sync::broadcast;

const BUS_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Arc<DomainEvent>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }

    pub fn publish(&self, event: DomainEvent) {
        let _ = self.sender.send(Arc::new(event));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<DomainEvent>> {
        self.sender.subscribe()
    }
}
