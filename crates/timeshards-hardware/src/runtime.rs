use crate::config::{
    hardware_adapter_configured, set_effective_hardware_adapter,
};
use crate::external::ExternalGateway;
use crate::gateway::{HardwareEvent, HardwareGateway};
use crate::simulator::{channel, HardwareEventSender, SimulatorGateway};
use tracing::warn;

/// Started hardware: API simulator handle, worker receiver, shared inject sender.
pub struct HardwareBootstrap {
    pub gateway: SimulatorGateway,
    pub events_rx: tokio::sync::mpsc::UnboundedReceiver<HardwareEvent>,
    pub inject: HardwareEventSender,
}

/// Start the configured adapter and wire the background credential worker channel.
pub async fn bootstrap_hardware() -> anyhow::Result<HardwareBootstrap> {
    let (tx, rx) = channel();
    let inject = tx.clone();
    let configured = hardware_adapter_configured();

    let gateway = match configured {
        "sim" => {
            set_effective_hardware_adapter("sim");
            let sim = SimulatorGateway::new(tx);
            sim.start().await?;
            sim
        }
        "external" => {
            set_effective_hardware_adapter("external");
            let ext = ExternalGateway::new(tx);
            ext.start().await?;
            SimulatorGateway::detached()
        }
        other => {
            warn!(
                configured = other,
                "unknown TIMESHARDS_HW_ADAPTER — falling back to sim"
            );
            set_effective_hardware_adapter("sim");
            let sim = SimulatorGateway::new(tx);
            sim.start().await?;
            sim
        }
    };

    Ok(HardwareBootstrap {
        gateway,
        events_rx: rx,
        inject,
    })
}
