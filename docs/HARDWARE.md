# Hardware integration (AI TimeShards)

Physical badge readers are **not** connected yet. Development uses the in-process **simulator** (`timeshards-hardware`) and REST simulate-scan endpoints.

## Architecture

```
Reader (future) → HardwareGateway adapter → mpsc channel → API worker → process_credential()
UI / scripts      → POST /api/v1/access/simulate-scan  ─────────────→ process_credential()  (once)
UI / scripts      → POST /api/v1/access/me/simulate-scan ───────────→ process_credential()  (once)
Tests / admin     → POST /api/v1/access/hardware-present ──────────→ channel → worker → process_credential()  (once)
```

- **Domain logic** lives in `timeshards-api` (`process_credential`, access rules, anti-passback).
- **Adapters** implement `HardwareGateway` in `crates/timeshards-hardware` and emit `HardwareEvent::CredentialPresented`.
- The Tauri server and headless `timeshards-api` binary spawn a background task that reads the channel and calls `process_credential` for each injected event.

## Rules for new adapters

1. **One path per scan** — Either route through the hardware channel **or** call `process_credential` from a dedicated API hook, never both for the same presentation. REST simulate endpoints intentionally **do not** call `SimulatorGateway::inject` (see comments in `routes/access.rs`) to avoid duplicate events and broken occupancy (deny from anti-passback overwriting grant).

2. **Normalized payload** — Map vendor data to `RawCredentialPresentation { reader_id, credential_uid, occurred_at }`. `reader_id` must match a configured door/reader in the DB (simulator uses `sim.reader.main` / `sim.reader.main.out`).

3. **Detached simulator** — Use `SimulatorGateway::detached()` when no background worker is needed (channel consumer omitted).

4. **No UI dependency** — Adapters run in the server process; clients keep using the API only.

## Adding a real adapter (sketch)

1. Create `crates/timeshards-hardware/src/<vendor>.rs` implementing `HardwareGateway`.
2. On card read, `tx.send(HardwareEvent::CredentialPresented(...))`.
3. Wire the adapter in `apps/server/src-tauri/src/lib.rs` and `crates/timeshards-api/src/bin/timeshards-api.rs` instead of (or alongside) `SimulatorGateway`, controlled by env e.g. `TIMESHARDS_HW_ADAPTER=sim|primion`.
4. Add integration tests with injected events; keep REST simulate for demos.
5. Test the worker path with `POST /api/v1/access/hardware-present` (same body as simulate-scan; poll events for result).

## Environment

| Variable | Values | Effect |
|----------|--------|--------|
| `TIMESHARDS_HW_ADAPTER` | `sim` (default) | In-process simulator; `inject` on `SimulatorGateway` feeds the worker |
| | `external` / `primion` | `ExternalGateway` + worker channel; REST simulate and optional TCP ingest |
| `TIMESHARDS_HW_TCP_ADDR` | e.g. `127.0.0.1:47831` | **External adapter only:** listen for newline-delimited JSON (see below). Unset = no TCP listener. |

`GET /api/v1/health` returns `hardware_adapter` (active: `sim` or `external`). If `TIMESHARDS_HW_ADAPTER` is invalid, the server runs `sim` and sets `hardware_adapter_configured` to `unknown`.

### TCP ingest (external adapter)

With `TIMESHARDS_HW_ADAPTER=external` and `TIMESHARDS_HW_TCP_ADDR` set, each accepted TCP connection may send **one JSON object per line**:

JSON (preferred):

```json
{"reader_id":"sim.reader.main","credential_uid":"DEMO-0002"}
```

Optional `occurred_at` (RFC3339).

Compact forwarder line (same channel):

```text
sim.reader.main;DEMO-0002
```

Door state (updates `doors.status`, shows on dashboard):

```json
{"kind":"door","door_id":"<door-uuid>","state":"alarm"}
```

```text
door;<door-uuid>;forced_open
```

Allowed `state`: `closed`, `open`, `forced_open`, `alarm`.

Reader offline (audit log + warning; no access event):

```json
{"kind":"reader_offline","reader_id":"sim.reader.main"}
```

```text
reader_offline;sim.reader.main
```

Lines are mapped on the hardware worker channel (same as `POST /api/v1/access/hardware-present` for credentials). Use for bridges, test scripts, or a future Primion forwarder — not a full OEM protocol yet.

Manual test: `.\scripts\send-hw-tcp.ps1 -CredentialUid DEMO-0002`

Before pilot: with API running, `npm run verify:doors` prints door UUIDs and `reader_id` values for bridge configuration.

Full M4 loop (pilot env + external TCP + test scan):

```powershell
$env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"
npm run api:hw-pilot   # terminal 1
npm run hw:pilot       # terminal 2 (after health shows external)
```

## Simulator readers (seed)

| `reader_id` | Role |
|-------------|------|
| `sim.reader.main` | Entry (in) |
| `sim.reader.main.out` | Exit (out) — required for anti-passback |

Demo credentials: `DEMO-ADMIN-001`, `DEMO-0002`, `DEMO-0003`.

## Bridge deployment (pilot)

Use this when one physical reader is forwarded over TCP into TimeShards (`TIMESHARDS_HW_ADAPTER=external`). A small **bridge** process on the site PC reads the vendor SDK or serial line and writes newline-delimited lines to the API listener.

### Checklist

1. **API** — Run headless or Tauri server with:
   - `TIMESHARDS_HW_ADAPTER=external`
   - `TIMESHARDS_HW_TCP_ADDR=127.0.0.1:47831` (or site LAN bind if the bridge runs on another host; firewall accordingly)
2. **Doors in DB** — Admin → Zutritt: door has `reader_in_id` / `reader_out_id` matching what the bridge sends (seed uses `sim.reader.main` / `sim.reader.main.out` for demos).
3. **Bridge** — Forwards only **one event per physical scan**; maps card UID to `credential_uid` and reader to `reader_id` exactly as configured.
4. **Smoke** — With API up:
   ```powershell
   cd m:\Data\Projects\ai_timeshards
   $env:TIMESHARDS_HW_ADAPTER = 'external'
   $env:TIMESHARDS_HW_TCP_ADDR = '127.0.0.1:47831'
   npm run api
   # second shell:
   .\scripts\send-hw-tcp.ps1 -CredentialUid DEMO-0002
   ```
   Poll `GET /api/v1/access/events?limit=5` — expect `grant` on first entry scan; second entry without exit → `deny` (anti-passback).
5. **Health** — `GET /api/v1/health` shows `hardware_adapter: external` and `hardware_tcp_listen` when the listener is bound.
6. **Restart** — After API restart, confirm bridge reconnects and door IDs in TCP `door;…` lines still match UUIDs in the DB (stale IDs fail silently in UI lists).

### Fail-closed reminder

Access without an explicit **Allow** rule for the employee’s zone is denied. Pilot sites should verify each employee has badge + zone rule (Personal → Setup or Zutritt → Regel) before go-live. Zones with no rules do not grant access.

### Door state from bridge

Optional lines update dashboard door status without a credential event:

```text
door;<door-uuid>;closed
```

See TCP ingest above for JSON equivalent.

## References

- Trait: `crates/timeshards-hardware/src/gateway.rs`
- Access API: `crates/timeshards-api/src/routes/access.rs`
- REST reference: [API.md](./API.md)
