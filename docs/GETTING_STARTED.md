# Getting started with AI TimeShards

This guide walks you through running the **Server** (admin + API) and **Client** (daily use) on Windows. For the full API list, see [README.md](../README.md).

## What you get

- **TimeShards Server** — desktop app with admin UI, embedded REST API, SQLite database.
- **TimeShards Client** — desktop app for clock-in/out, absences, approvals (role-dependent), and access self-service.

Hardware is simulated only (badge scan simulator); no physical readers yet. See [HARDWARE.md](HARDWARE.md) for adapter integration rules.

## Prerequisites

| Requirement | Notes |
|-------------|--------|
| [Rust](https://rustup.rs/) (stable) | On Windows, also install **Visual Studio C++ Build Tools** |
| [Node.js](https://nodejs.org/) 20+ | Includes `npm` |
| [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) | Usually already on Windows 11 |

Verify in PowerShell:

```powershell
node -v
npm -v
cargo -V
```

## First-time setup

From the repository root (adjust the path if yours differs):

```powershell
cd m:\Data\Projects\ai_timeshards
npm install
```

This installs dependencies for both apps via npm workspaces.

## Start everything (easiest)

One command opens the server in a **new window**, waits until the API responds, then starts the client in the current window:

```powershell
cd m:\Data\Projects\ai_timeshards
.\scripts\start_all.ps1
```

With the server running, verify the API:

```powershell
.\scripts\smoke-test.ps1
```

Without the Tauri server (headless API + smoke in one step):

```powershell
npm run smoke:api
# or: .\scripts\smoke-with-api.ps1
```

Uses `cargo run --bin timeshards-api` and SQLite at `.data/smoke.db`.

Headless API only (development):

```powershell
npm run api
# or .\scripts\run-api.ps1
```

## Start the server only

**Option A — helper script (recommended):**

```powershell
cd m:\Data\Projects\ai_timeshards
.\scripts\start_server.ps1
```

**Option B — manual:**

```powershell
cd m:\Data\Projects\ai_timeshards\apps\server
npm run tauri dev
```

The first build can take several minutes. Later starts are faster.

When the window opens:

- **Login:** `admin` / `admin`
- **API base URL:** `http://127.0.0.1:47821`
- **Health check:** open `http://127.0.0.1:47821/api/v1/health` in a browser

The server stores its database under the Tauri app data directory (shown in the server UI after login).

## Start the client (optional)

Use a **second** PowerShell window while the server keeps running.

**Option A — helper script:**

```powershell
cd m:\Data\Projects\ai_timeshards
.\scripts\start_client.ps1
```

**Option B — manual:**

```powershell
cd m:\Data\Projects\ai_timeshards\apps\client
npm run tauri dev
```

On the client **settings** screen, set:

- **Server URL:** `http://127.0.0.1:47821` (same machine)
- Then log in with `demo` / `demo` (employee) or `manager` / `demo` (approvals)

## Default demo data

Created automatically on first server start:

| Item | Value |
|------|--------|
| Site | Hauptstandort |
| Zone | Büro |
| Door | Haupteingang |
| Demo badge (admin) | `DEMO-ADMIN-001` |
| Policy | DE / ArbZG-oriented v0 |

### Demo logins

Added on every server start if missing (for client and role testing):

| Username | Password | Role | Personalnr. | Badge UID |
|----------|----------|------|-------------|-----------|
| `admin` | `admin` | System admin | `0001` | `DEMO-ADMIN-001` |
| `demo` | `demo` | Mitarbeiter | `0002` | `DEMO-0002` |
| `manager` | `demo` | Vorgesetzte/r | `0003` | `DEMO-0003` |

Use **`demo` / `demo`** in the **Client** to test clock-in, own timesheets, and Zutritt. Demo data includes **standard work calendar** assignment, **Mo–Fr shifts**, **yesterday’s punches**, a **draft timesheet** for the current week (rebuild for Ist/Soll/Saldo), and a **pending Urlaub** request. Use **`manager` / `demo`** for Freigaben — you should see **1 pending timesheet** (admin) and **1 pending absence** (demo). The server UI stays on **`admin` / `admin`**.

Change default passwords before any real deployment.

### Production / staging

| Variable | Effect |
|----------|--------|
| `TIMESHARDS_ADMIN_PASSWORD` | Initial `admin` password when the database is first created (empty DB only) |
| `TIMESHARDS_DISABLE_DEMO=1` | Do not create or refresh `demo` / `manager` users or sample week data on startup; **blocks login** with `admin`/`admin`, `demo`/`demo`, `manager`/`demo` |
| `TIMESHARDS_BLOCK_DEFAULT_PASSWORDS=1` | Reject built-in passwords at login even when demo seeding is still on (staging hardening) |
| `TIMESHARDS_DB` | SQLite path (headless API) |
| `TIMESHARDS_API_HOST` / `TIMESHARDS_API_PORT` | API bind (default `0.0.0.0:47821`) |
| `TIMESHARDS_HW_ADAPTER` | `sim` (default) or `external` / `primion` (TCP JSON ingest optional) |
| `TIMESHARDS_HW_TCP_ADDR` | e.g. `127.0.0.1:47831` — external adapter only; see [HARDWARE.md](./HARDWARE.md) |

See [.env.example](../.env.example) for a copy-paste template.

`GET /api/v1/health` returns `demo_seeding_enabled: true` when demo seeding is still active (check after deploy).

**Recommended production cutover:** set `TIMESHARDS_ADMIN_PASSWORD` on first start (empty DB), or run once with demo seeding on, change the `admin` password in the server UI, then restart with `TIMESHARDS_DISABLE_DEMO=1`. Verify with `npm run smoke:production`.

## Server UI tour (admin)

After login, use the tabs:

### Übersicht

- Pending timesheets and absences (click KPI → jump to Zeit/Abwesenheit with filter); **Im Gebäude** / **Schichten** / **Tür-Alerts** jump to Zutritt or Zeit.
- **Eingestempelt** count plus named list with last punch time.
- **Im Gebäude** — people counted inside zones (from access events).
- **Tür-Alerts** — doors that are open, forced open, or in alarm; **Zurücksetzen** closes the door and jumps to Zutritt.
- **Zeitbasis** — if active employees lack a work calendar or draft timesheets have no Soll this week, amber KPIs appear; **Zeitbasis reparieren** assigns the standard calendar and rebuilds the current KW (`POST /api/v1/admin/foundation-fix`).

### Zeit

- **Arbeitskalender & Tagesmodelle** — Sollzeit from the work calendar (Mo–Fr 8h seed, holidays, per-employee assignment, optional **Umschaltplan**, **KW kopieren**). Edit **Tagesmodell** (Soll minutes, Gleitzeit band) — saves via API and rebuilds affected timesheets. See [TIME_MODEL.md](./TIME_MODEL.md) and [FOUNDATION.md](./FOUNDATION.md).
- **Zeitkonten** — Gleitzeit/Überstunden after timesheet approve; **Monatsabschluss** when all weeks in the month are approved (reconciliation only if weekly postings differ).
- **Abrechnungsregel** — optional **Gleitzeit beim Stempeln erzwingen** (otherwise punch returns an advisory only).
- **Stundenzettel** show **Ist / Soll / Saldo**; **Tagesdetails** after rebuild (calendar + absence credit per day).
- **Stundenzettel neu berechnen** uses the week shown in the shift calendar (calendar-aware, not punch-only).
- Clock events, shifts, timesheet submit/approve/reject.
- Filter Stundenzettel by status and (managers) by employee; **Alle N freigeben** for pending batch; **Team-Entwürfe** for colleagues’ draft/rejected sheets.
- **Wochenvorlagen (geplante Schichten)** — recurring templates for shift planning; **Woche anwenden** generates shift instances (separate from Soll).
- Approving **Abwesenheit** triggers timesheet rebuild for affected weeks.
- **Geplante veröffentlichen** (count on button) publishes all `planned` shifts in the visible calendar week; orange nav badge = planned count.
- **Heute** resets the shift calendar to the current week.
- Exports: timesheet CSV / HTML (print to PDF from the browser).

### Abwesenheit

- Create and approve absence requests; conflict pre-check.
- Filter by status; **Alle N freigeben** for pending batch.

### Zutritt

- Zones, doors, badges, **Zutrittsregeln** (allow employee → zone; **Mo–Fr 08–18** toggles schedule; **Entfernen** deletes a rule).
- **Belegung** — who is inside each zone.
- **Türen** — set status (Zu / Auf / Offen / Alarm).
- **Simulator** — scan with credential UID `DEMO-ADMIN-001`; readers `sim.reader.main` (in) and `sim.reader.main.out` (out).
- **Zutritt CSV** / **Zutritt HTML/PDF** — company-wide access log (admin/HR/manager/security); optional date range above the buttons.

### Personal / System

- **Neuer Benutzer** — creates login + employee record.
- **Mitarbeiter (ohne Login)** — HR record; optional badge + Büro-Zutrittsregel + **Arbeitskalender** (default on); **Bearbeiten** / **Badge ausstellen** / **Badge + Zutritt** / **Arbeitskalender** button when missing; filters **Nur ohne Badge oder Zutritt** and **Nur ohne Arbeitskalender**; **Benutzer verknüpfen** für Logins ohne Profil.
- Users, employees list, audit log, password change.

## Client UI tour

Log in as **`demo` / `demo`** (employee) or **`manager` / `demo`** (approvals). Pillars: **Zeit**, **Abwesenheit**, **Freigaben** (managers), **Zutritt**, **Konto**. Sidebar badges show open approvals and draft timesheets; **manager** lands on **Freigaben** when the queue is non-empty.

- Clock in/out, break start/end — **Gehen** and **Pause Ende** recalculate the current week timesheet automatically (Ist/Soll/Saldo).
- Sidebar and Zeit tab show **KW Ist · Soll · Saldo** via `work-summary.current_week` when a work calendar is assigned; warning if **Kein Arbeitskalender**.
- Own shifts and timesheets (submit draft/rejected); filter lists by status; **CSV exportieren** (scoped to your rows).
- Absence requests; managers approve pending items (bulk **Alle N** on Freigaben). **Team-Entwürfe** lists colleagues’ draft/rejected timesheets with per-row **Einreichen**.
- Own badges and simulate scan on **Zutritt** (`DEMO-0002` for demo, `DEMO-0003` for manager).
- **Manuell neu berechnen** (client) if Soll is still missing — normally not needed after stamping; only your own week, not the whole company.
- **Konto** — change server URL or password (password change logs you out).

## API without the UI

```powershell
# Health
Invoke-RestMethod http://127.0.0.1:47821/api/v1/health

# Login
$login = @{ username = "admin"; password = "admin" } | ConvertTo-Json
$res = Invoke-RestMethod -Method Post `
  -Uri http://127.0.0.1:47821/api/v1/auth/login `
  -Body $login -ContentType "application/json"
$token = $res.token

# Authenticated request
$headers = @{ Authorization = "Bearer $token" }
Invoke-RestMethod -Uri http://127.0.0.1:47821/api/v1/admin/dashboard -Headers $headers
```

Full API reference: [API.md](./API.md).

## Another PC on the LAN

1. On the **server PC**, note its LAN IP (e.g. `192.168.1.50`).
2. Open the firewall **once**, as Administrator:

   ```powershell
   cd m:\Data\Projects\ai_timeshards\scripts
   .\open-firewall.ps1
   ```

3. On **clients**, set server URL to `http://192.168.1.50:47821` (not `127.0.0.1`).
4. The server admin UI lists suggested client URLs when available.

## Script reference

| Script | Purpose |
|--------|---------|
| `scripts/start_all.ps1` | Server in new window → wait for health → client |
| `scripts/start_server.ps1` | `npm install` if needed, then `tauri dev` for server |
| `scripts/start_client.ps1` | Same for client (run after server) |
| `scripts/open-firewall.ps1` | Inbound TCP 47821 (Domain + Private); needs elevation |
| `scripts/smoke-test.ps1` | Health + login + dashboard (server must be up) |
| `scripts/smoke-with-api.ps1` | Fresh `.data/smoke.db`, headless API, full smoke |
| `scripts/run-api.ps1` | Headless API only (`TIMESHARDS_DB` optional) |
| `scripts/check.ps1` | `cargo check` + `svelte-check` (server + client) |

Both start scripts accept `-SkipNpmInstall` if dependencies are already installed.

From repo root: `npm run smoke:api`, `npm run api`, `npm run check:all`.

Equivalent npm commands from repo root:

```powershell
npm run tauri:server -- dev
npm run tauri:client -- dev
```

## Troubleshooting

### `cargo` or `npm` not found

- Restart PowerShell after installing Rust or Node.
- Scripts prepend `%USERPROFILE%\.cargo\bin` to `PATH` for cargo.

### First `tauri dev` is very slow

- Normal: compiles Rust crates and Tauri. Wait for “Finished” / window to open.

### Client cannot connect

- Server must be running first.
- URL must include `http://` and port `:47821`.
- From another machine: use server LAN IP + firewall rule.

### Login fails after schema changes

- Restart the server so migrations run.
- Default user remains `admin` / `admin` on fresh DB.

### Access scan always denied

- Admin badge: `DEMO-ADMIN-001`; demo employee: `DEMO-0002` (Client **demo** / **demo**).
- Ensure an **allow** rule exists for that employee and zone (seed creates one).
- For occupancy, scan **Eingang** (`sim.reader.main`) then **Ausgang** (`sim.reader.main.out`). A second **Eingang** scan without exit triggers **Anti-Passback**.
- If **Im Gebäude** stays at 0 after a granted entry, restart the server (older builds processed each simulate-scan twice and could leave a deny as the latest zone event).

### Port 47821 already in use

- Close the other server instance or find the process using the port.

## OpenAPI

Import into [Swagger Editor](https://editor.swagger.io/) or similar:

- URL (server running): `http://127.0.0.1:47821/api/v1/openapi.json`
- File: [docs/openapi.json](openapi.json) (subset of routes; full narrative in [API.md](API.md))

## What is not documented here yet

- Production deployment and hardening
- Real hardware adapters

For product vision, see [ROADMAP.md](../ROADMAP.md) and [ROADMAP_DETAILS.md](../ROADMAP_DETAILS.md).
