use std::sync::Arc;
use timeshards_hardware::HardwareEvent;
use tracing::warn;

use timeshards_db::audit::write_audit;

use crate::routes::access::{process_credential, process_door_state};
use crate::AppState;

/// Background task: hardware channel → access evaluation and door status updates.
pub fn spawn_credential_worker(
    state: Arc<AppState>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<HardwareEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                HardwareEvent::CredentialPresented(p) => {
                    if let Err(e) =
                        process_credential(&state, &p.credential_uid, &p.reader_id).await
                    {
                        warn!(
                            credential_uid = %p.credential_uid,
                            reader_id = %p.reader_id,
                            error = %e.body.message,
                            "hardware credential processing failed"
                        );
                    }
                }
                HardwareEvent::DoorStateChanged {
                    door_id,
                    state: door_state,
                    ..
                } => {
                    if let Err(e) =
                        process_door_state(&state, &door_id.to_string(), &door_state).await
                    {
                        warn!(
                            door_id = %door_id,
                            status = %door_state,
                            error = %e.body.message,
                            "hardware door state update failed"
                        );
                    }
                }
                HardwareEvent::ReaderOffline { reader_id, .. } => {
                    warn!(reader_id = %reader_id, "reader offline (hardware)");
                    if let Err(e) = write_audit(
                        &state.db,
                        "hardware",
                        None,
                        "reader_offline",
                        "reader",
                        None,
                        Some(reader_id.as_str()),
                        None,
                        None,
                    )
                    .await
                    {
                        warn!(
                            reader_id = %reader_id,
                            error = %e,
                            "failed to audit reader offline"
                        );
                    }
                }
                HardwareEvent::Heartbeat { device_id, .. } => {
                    tracing::debug!(device_id = %device_id, "hardware heartbeat");
                }
            }
        }
    });
}
