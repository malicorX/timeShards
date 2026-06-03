# AI TimeShards

Modular desktop system for **time tracking** and **access control** (Germany-first, worldwide-ready).

**Current status:** [STATUS.md](STATUS.md) · **v0.2.2** ([release notes](docs/releases/v0.2.2.md))

- **TimeShards Server** — Windows desktop app: configuration, API, database, audit.
- **TimeShards Client** — Windows desktop app: connects to the server for daily work (clock-in/out, access events).

## Architecture

```
crates/
  timeshards-core/      # Permissions, events, shared types
  timeshards-kernel/    # Shard registry, event bus
  timeshards-db/        # SQLite schema, migrations, seed
  timeshards-api/       # Axum REST API
  timeshards-hardware/  # Gateway trait + simulator (no real devices yet)
apps/
  server/               # Tauri + Svelte admin shell
  client/               # Tauri + Svelte operator shell
```

Clients are **online-first**: they call the server API (`http://<host>:47821` by default). Hardware integrates later via `timeshards-hardware` without changing domain logic.

## Prerequisites

1. [Rust](https://rustup.rs/) (stable) + Visual Studio **C++ Build Tools** (Windows)
2. [Node.js](https://nodejs.org/) 20+
3. [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (usually preinstalled on Windows 11)

## Documentation

- **[docs/GETTING_STARTED.md](docs/GETTING_STARTED.md)** — install, run, UI tour, LAN, troubleshooting
- **[docs/API.md](docs/API.md)** — REST API reference
- **[docs/TIME_MODEL.md](docs/TIME_MODEL.md)** — work calendars, Soll/Ist evaluation
- **[docs/FOUNDATION.md](docs/FOUNDATION.md)** — time-model foundation status
- **[docs/PRODUCTION.md](docs/PRODUCTION.md)** — production go-live (env, checklist, verify)
- **[docs/PAYROLL_EXPORT.md](docs/PAYROLL_EXPORT.md)** — Lohn- + Abwesenheiten-CSV, Monats-Paket
- **[docs/PILOT.md](docs/PILOT.md)** — first customer cutover checklist
- **[docs/PHASE2.md](docs/PHASE2.md)** — post-v1 tracks (DATEV, hardware, …)
- **[CONTRIBUTING.md](CONTRIBUTING.md)** — verify before push
- **[docs/FOUNDATION_CHECKLIST.md](docs/FOUNDATION_CHECKLIST.md)** — admin checklist for Kalender/Soll
- **[docs/openapi.json](docs/openapi.json)** — OpenAPI 3.0 subset (live: `GET /api/v1/openapi.json`)
- **[docs/HARDWARE.md](docs/HARDWARE.md)** — hardware gateway and reader integration
- **[docs/README.md](docs/README.md)** — documentation index
- **[AGENTS.md](AGENTS.md)** — notes for coding agents (run commands, layout)

## Quick start

```powershell
cd m:\Data\Projects\ai_timeshards
npm install

# Server + Client (server opens in a second window)
.\scripts\start_all.ps1

# Or separately:
.\scripts\start_server.ps1
.\scripts\start_client.ps1
```

Or manually: `cd apps\server` / `apps\client` and `npm run tauri dev`. See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md).

Default credentials after first start (change in production):

| App | Username | Password | Role |
|-----|----------|----------|------|
| Server | `admin` | `admin` | System admin |
| Client | `demo` | `demo` | Employee (PN 0002, badge `DEMO-0002`) |
| Client | `manager` | `demo` | Manager (approvals, badge `DEMO-0003`) |

Verify API without UI:

```powershell
npm run smoke:api
npm run smoke:production   # TIMESHARDS_DISABLE_DEMO + default passwords blocked
npm run smoke:strict       # TIMESHARDS_BLOCK_DEFAULT_PASSWORDS (demo seed still on)
npm run smoke:hw-external  # external adapter + TCP credential/door ingest
npm run smoke:hw-unknown   # invalid TIMESHARDS_HW_ADAPTER → sim fallback
# or: .\scripts\run-api.ps1   # then .\scripts\smoke-test.ps1
npm run check:all              # cargo check + svelte-check
npm run verify:foundation      # timeshards-db tests + smoke:api (work calendar / Soll)
npm run verify:all             # check:all + verify:foundation (pre-push)
```

Environment template: [.env.example](.env.example). Hardware bridge: [docs/HARDWARE.md](docs/HARDWARE.md).

The API listens on **`0.0.0.0:47821`** by default (all interfaces). The server UI shows **Client-URLs** including your LAN IP.

On each **client**, set the server URL to `http://<server-pc-ip>:47821` (not `127.0.0.1` from other machines).

## API (excerpt)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health` | Health check |
| POST | `/api/v1/auth/login` | Login |
| GET | `/api/v1/auth/me` | Current user |
| GET | `/api/v1/me/work-summary` | Clock status, own drafts/absences, manager approval counts |
| POST | `/api/v1/admin/employees` | Create employee (optional badge, auto PN) |
| POST | `/api/v1/time/clock-in` | Clock in |
| POST | `/api/v1/time/clock-out` | Clock out |
| POST | `/api/v1/time/break-start` | Start break (pause) |
| POST | `/api/v1/time/break-end` | End break |
| POST | `/api/v1/time/corrections` | Manual time correction (manager/HR) |
| GET | `/api/v1/time/events?employee_id=` | List punches (managers: any employee) |
| GET | `/api/v1/access/zones` | List zones |
| POST | `/api/v1/access/simulate-scan` | Simulate badge scan |
| GET/POST | `/api/v1/access/rules` | Zone access rules (allow per employee) |
| PATCH | `/api/v1/access/rules/{id}` | Update schedule / validity |
| DELETE | `/api/v1/access/rules/{id}` | Remove access rule |
| POST | `/api/v1/access/badges/{id}/revoke` | Revoke badge |
| GET | `/api/v1/admin/audit` | Audit log (last 100 entries) |
| GET | `/api/v1/access/occupancy` | Who is inside each zone |
| GET | `/api/v1/access/me` | Own badges + recent access events |
| POST | `/api/v1/access/me/simulate-scan` | Simulate scan with own badge |
| GET/POST | `/api/v1/admin/users` | List / create users |
| GET | `/api/v1/admin/employees` | List employees |
| POST | `/api/v1/auth/change-password` | Change own password |
| GET/POST | `/api/v1/time/shifts` | Shifts |
| GET | `/api/v1/admin/dashboard` | KPIs (clocked-in, pending approvals, `planned_shifts_this_week`) |
| POST | `/api/v1/admin/employees/{id}/grant-zone-access` | Default Büro allow rule |
| GET | `/api/v1/time/shifts?from=&to=` | Shifts in date range (employees: own only) |
| POST | `/api/v1/time/shifts/{id}/publish` | Publish planned shift |
| POST | `/api/v1/time/shifts/publish-planned` | Publish all planned shifts in week |
| GET | `/api/v1/admin/policy` | Active time policy limits |
| POST | `/api/v1/time/shifts/{id}/cancel` | Cancel shift |
| GET | `/api/v1/absences/conflicts` | Pre-check absence overlap |
| GET | `/api/v1/time/shifts/conflicts` | Pre-check shift overlap |
| GET | `/api/v1/time/timesheets` | Timesheets |
| POST | `/api/v1/time/timesheets/rebuild` | Recompute weekly timesheets |
| POST | `/api/v1/time/timesheets/{id}/submit` | Submit timesheet for approval |
| POST | `/api/v1/time/timesheets/{id}/approve` | Approve timesheet (manager) |
| POST | `/api/v1/time/timesheets/{id}/reject` | Reject timesheet (manager) |
| GET/POST | `/api/v1/absences` | List / create absence requests |
| POST | `/api/v1/absences/{id}/approve` | Approve absence |
| GET | `/api/v1/reports/timesheets/export?format=csv\|html` | Export approved timesheets |
| GET | `/api/v1/reports/access/export?format=csv\|html` | Export access event log |
| POST | `/api/v1/time/shift-templates/{id}/deactivate` | Deactivate shift template |
| GET/POST | `/api/v1/time/shift-templates` | Recurring shift templates |
| POST | `/api/v1/time/shift-templates/apply-week` | Generate week shifts from templates |
| POST | `/api/v1/access/doors/{id}/status` | Set door status (closed/open/alarm) |
| POST | `/api/v1/access/zones` | Create zone |
| POST | `/api/v1/access/doors` | Create door |
| GET/POST | `/api/v1/access/badges` | List / issue badges |

### Helper scripts (Windows)

| Script | Purpose |
|--------|---------|
| `scripts/start_all.ps1` | Start server (new window) + client after API is up |
| `scripts/start_server.ps1` | Start server (Tauri dev) |
| `scripts/start_client.ps1` | Start client (Tauri dev) |
| `scripts/open-firewall.ps1` | Allow LAN access to port 47821 (Administrator) |
| `scripts/smoke-test.ps1` / `npm run smoke` | API smoke test (server must run) |
| `scripts/check.ps1` / `npm run check:all` | Rust + Svelte typecheck |

### LAN firewall (Windows)

Run once on the server PC as Administrator:

```powershell
.\scripts\open-firewall.ps1
```

## Default data

Seeded on first run: site **Hauptstandort**, zone **Büro**, door **Haupteingang**, badge `DEMO-ADMIN-001`, DE policy pack (ArbZG-oriented v0).

## Roadmap docs

- `ROADMAP.md` / `ROADMAP_DETAILS.md` — product vision
- `deep-research-report*.md` — research and build specs

## Next steps (post v1 foundation)

- PostgreSQL option for multi-site central DB
- DATEV bridge (Lohn-CSV export exists as interim)
- Full-year calendar editor beyond KW copy + generate-year
- Automatic stamp ↔ building access sync (process + KPI today)
- Production hardware adapters behind `HardwareGateway`
- Mobile / SaaS (later)
