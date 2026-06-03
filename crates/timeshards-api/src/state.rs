use sqlx::SqlitePool;
use std::sync::Arc;
use timeshards_hardware::{HardwareEventSender, SimulatorGateway};
use timeshards_kernel::ShardRegistry;

pub struct AppState {
    pub db: SqlitePool,
    pub registry: Arc<tokio::sync::Mutex<ShardRegistry>>,
    pub hardware_sim: Arc<SimulatorGateway>,
    /// Shared channel for adapter / test inject (one credential event → worker → process_credential).
    pub hardware_inject: HardwareEventSender,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        registry: ShardRegistry,
        hardware_sim: SimulatorGateway,
        hardware_inject: HardwareEventSender,
    ) -> Self {
        Self {
            db,
            registry: Arc::new(tokio::sync::Mutex::new(registry)),
            hardware_sim: Arc::new(hardware_sim),
            hardware_inject,
        }
    }
}
