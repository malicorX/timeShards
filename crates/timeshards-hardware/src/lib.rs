pub mod config;
pub mod external;
pub mod gateway;
pub mod runtime;
pub mod simulator;
pub mod tcp_ingest;

pub use config::{
    hardware_adapter_active, hardware_adapter_configured, hardware_adapter_id,
    hardware_tcp_listen_addr, is_simulator_adapter, set_effective_hardware_adapter,
};
pub use gateway::{HardwareEvent, HardwareGateway, RawCredentialPresentation};
pub use runtime::{bootstrap_hardware, HardwareBootstrap};
pub use simulator::HardwareEventSender;
pub use simulator::{channel, SimulatorGateway};
