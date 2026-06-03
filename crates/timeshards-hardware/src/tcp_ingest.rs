//! Line-delimited ingest for external readers (bridge until a Primion/OEM SDK exists).
//!
//! Credential JSON: `{"reader_id":"sim.reader.main","credential_uid":"DEMO-0002"}`
//! Credential compact: `sim.reader.main;DEMO-0002`
//! Door JSON: `{"kind":"door","door_id":"<uuid>","state":"alarm"}`
//! Door compact: `door;<door_id>;alarm`
//! Reader offline JSON: `{"kind":"reader_offline","reader_id":"sim.reader.main"}`
//! Reader offline compact: `reader_offline;sim.reader.main`

use crate::gateway::{HardwareEvent, RawCredentialPresentation};
use crate::simulator::HardwareEventSender;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct TcpJsonLine {
    #[serde(default)]
    kind: Option<String>,
    reader_id: Option<String>,
    credential_uid: Option<String>,
    door_id: Option<String>,
    state: Option<String>,
    #[serde(default)]
    occurred_at: Option<DateTime<Utc>>,
}

pub fn parse_hardware_line(line: &str) -> anyhow::Result<HardwareEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        anyhow::bail!("empty line");
    }
    if trimmed.starts_with('{') {
        return parse_json_line(trimmed);
    }
    if let Some(rest) = trimmed.strip_prefix("door;") {
        return parse_door_compact(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("reader_offline;") {
        return parse_reader_offline_compact(rest);
    }
    if let Some((reader_id, credential_uid)) = trimmed.split_once(';') {
        let presentation = presentation_from_parts(
            reader_id.trim().to_string(),
            credential_uid.trim().to_string(),
            None,
        )?;
        return Ok(HardwareEvent::CredentialPresented(presentation));
    }
    anyhow::bail!("expected JSON, reader_id;credential_uid, or door;door_id;status");
}

fn parse_json_line(trimmed: &str) -> anyhow::Result<HardwareEvent> {
    let row: TcpJsonLine =
        serde_json::from_str(trimmed).context("invalid JSON hardware line")?;
    let at = row.occurred_at.unwrap_or_else(Utc::now);
    if row.kind.as_deref() == Some("reader_offline") {
        let reader_id = row
            .reader_id
            .filter(|s| !s.is_empty())
            .context("reader_id required")?;
        return Ok(HardwareEvent::ReaderOffline {
            reader_id,
            occurred_at: at,
        });
    }
    if row.kind.as_deref() == Some("door")
        || (row.door_id.is_some() && row.state.is_some() && row.credential_uid.is_none())
    {
        let door_id = row
            .door_id
            .filter(|s| !s.is_empty())
            .context("door_id required")?;
        let state = row.state.filter(|s| !s.is_empty()).context("state required")?;
        let door_uuid = Uuid::parse_str(&door_id)
            .with_context(|| format!("door_id must be a UUID, got {door_id}"))?;
        return Ok(HardwareEvent::DoorStateChanged {
            door_id: door_uuid,
            state,
            occurred_at: at,
        });
    }
    let reader_id = row
        .reader_id
        .filter(|s| !s.is_empty())
        .context("reader_id required")?;
    let credential_uid = row
        .credential_uid
        .filter(|s| !s.is_empty())
        .context("credential_uid required")?;
    Ok(HardwareEvent::CredentialPresented(
        presentation_from_parts(reader_id, credential_uid, Some(at))?,
    ))
}

fn parse_reader_offline_compact(rest: &str) -> anyhow::Result<HardwareEvent> {
    let reader_id = rest.trim();
    if reader_id.is_empty() {
        anyhow::bail!("reader_id must be non-empty");
    }
    Ok(HardwareEvent::ReaderOffline {
        reader_id: reader_id.to_string(),
        occurred_at: Utc::now(),
    })
}

fn parse_door_compact(rest: &str) -> anyhow::Result<HardwareEvent> {
    let parts: Vec<&str> = rest.split(';').collect();
    if parts.len() != 2 {
        anyhow::bail!("door compact line: door;<door_id>;<status>");
    }
    let door_id = parts[0].trim();
    let state = parts[1].trim();
    if door_id.is_empty() || state.is_empty() {
        anyhow::bail!("door_id and status must be non-empty");
    }
    let door_uuid =
        Uuid::parse_str(door_id).with_context(|| format!("door_id must be a UUID, got {door_id}"))?;
    Ok(HardwareEvent::DoorStateChanged {
        door_id: door_uuid,
        state: state.to_string(),
        occurred_at: Utc::now(),
    })
}

fn presentation_from_parts(
    reader_id: String,
    credential_uid: String,
    occurred_at: Option<DateTime<Utc>>,
) -> anyhow::Result<RawCredentialPresentation> {
    if reader_id.is_empty() || credential_uid.is_empty() {
        anyhow::bail!("reader_id and credential_uid must be non-empty");
    }
    Ok(RawCredentialPresentation {
        reader_id,
        credential_uid,
        occurred_at: occurred_at.unwrap_or_else(Utc::now),
    })
}

/// Listen for newline-delimited hardware lines and push them on the hardware channel.
pub fn spawn_tcp_credential_listener(
    listen_addr: impl Into<String>,
    events: HardwareEventSender,
) -> JoinHandle<()> {
    let listen_addr = listen_addr.into();
    tokio::spawn(async move {
        let listener = match TcpListener::bind(&listen_addr).await {
            Ok(l) => l,
            Err(e) => {
                error!(addr = %listen_addr, error = %e, "TCP hardware ingest failed to bind");
                return;
            }
        };
        info!(addr = %listen_addr, "TCP hardware ingest listening (newline-delimited lines)");

        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    warn!(error = %e, "TCP accept failed");
                    continue;
                }
            };
            let tx = events.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    match parse_hardware_line(&line) {
                        Ok(event) => {
                            debug!(peer = %peer, ?event, "TCP hardware line");
                            if let Err(e) = tx.send(event) {
                                warn!(error = %e, "hardware channel closed");
                                break;
                            }
                        }
                        Err(e) => warn!(peer = %peer, error = %e, line = %line, "ignored TCP line"),
                    }
                }
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_credential_json() {
        let ev = parse_hardware_line(
            r#"{"reader_id":"sim.reader.main","credential_uid":"DEMO-0002"}"#,
        )
        .unwrap();
        assert!(matches!(ev, HardwareEvent::CredentialPresented(_)));
    }

    #[test]
    fn parses_credential_semicolon() {
        let ev = parse_hardware_line("sim.reader.main.out;DEMO-0002").unwrap();
        assert!(matches!(ev, HardwareEvent::CredentialPresented(_)));
    }

    #[test]
    fn parses_door_json() {
        let id = Uuid::new_v4();
        let line = format!(r#"{{"kind":"door","door_id":"{id}","state":"alarm"}}"#);
        let ev = parse_hardware_line(&line).unwrap();
        match ev {
            HardwareEvent::DoorStateChanged { state, .. } => assert_eq!(state, "alarm"),
            _ => panic!("expected door event"),
        }
    }

    #[test]
    fn parses_door_compact() {
        let id = Uuid::new_v4();
        let line = format!("door;{id};forced_open");
        let ev = parse_hardware_line(&line).unwrap();
        match ev {
            HardwareEvent::DoorStateChanged { state, .. } => assert_eq!(state, "forced_open"),
            _ => panic!("expected door event"),
        }
    }

    #[test]
    fn parses_reader_offline_compact() {
        let ev = parse_hardware_line("reader_offline;sim.reader.main").unwrap();
        match ev {
            HardwareEvent::ReaderOffline { reader_id, .. } => {
                assert_eq!(reader_id, "sim.reader.main");
            }
            _ => panic!("expected reader offline"),
        }
    }
}
