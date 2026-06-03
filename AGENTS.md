# AI TimeShards — agent notes

Germany-first desktop **time tracking + access control** (Primion-inspired). Online-first: Tauri 2 + Svelte 5 UIs call Axum API on port **47821**, SQLite via `timeshards-db`.

## Run locally (Windows)

```powershell
cd m:\Data\Projects\ai_timeshards
.\scripts\start_all.ps1          # server window + client
.\scripts\smoke-test.ps1        # API health + login (server must run)
npm run smoke                   # smoke-test.ps1 (server must run)
npm run smoke:api               # headless API + smoke (no Tauri)
npm run smoke:production        # DISABLE_DEMO + default-password login blocked
npm run smoke:strict            # BLOCK_DEFAULT_PASSWORDS with demo seed still on
npm run api                     # headless API only (scripts/run-api.ps1)
npm run check:all               # scripts/check.ps1 (cargo + svelte-check)
.\scripts\check.ps1              # same as check:all
npm run verify:foundation       # cargo test timeshards-db + smoke:api
npm run foundation:health       # GET /health time_foundation (API must run)
```

Default logins: `admin` / `admin` (server, badge `DEMO-ADMIN-001`), `demo` / `demo` (client employee, `DEMO-0002`), `manager` / `demo` (approvals, `DEMO-0003`). Override initial admin via `TIMESHARDS_ADMIN_PASSWORD` on empty DB. Demo accounts, badges, and week data are ensured on each server/API start via `ensure_demo_accounts` / `seed_demo_week_data` unless `TIMESHARDS_DISABLE_DEMO=1`.

CI: `.github/workflows/ci.yml` runs `cargo check` and `svelte-check` on push/PR.

## Layout

| Path | Role |
|------|------|
| `crates/timeshards-api` | REST routes, access eval, ArbZG policy |
| `crates/timeshards-db` | Migrations, seed |
| `apps/server` | Admin Tauri app |
| `apps/client` | Employee Tauri app |
| `docs/GETTING_STARTED.md` | User guide |
| `docs/API.md` | REST reference |
| `docs/TIME_MODEL.md` | Work calendar / Soll-Ist model |
| `docs/FOUNDATION.md` | Time foundation — implemented vs planned |
| `docs/FOUNDATION_CHECKLIST.md` | Admin go-live checklist for work calendar / Soll |
| `docs/openapi.json` | OpenAPI subset; served at `GET /api/v1/openapi.json` |

## Conventions

- PowerShell: use `;` not `&&`
- Minimal diffs; match existing German UI strings
- No commits unless the user asks
- Access: simulated badges only (`DEMO-*` UIDs); anti-passback via in/out readers; `TIMESHARDS_HW_ADAPTER=sim|external`; REST `simulate-scan`, channel `hardware-present` (poll `GET /access/events?since=`), external TCP lines (credential + door + reader_offline); see `docs/HARDWARE.md`
- New employees/users often get Büro zone allow rule via `grant_default_zone_access`; existing MA: `POST /admin/employees/{id}/grant-zone-access`
- Work calendar (Sollzeit): `grant_work_calendar` on `POST /admin/employees` (default); `POST /admin/employees/{id}/grant-work-calendar`; bulk `POST /admin/foundation-fix`; week bounds `GET /time/calendar-week`; see `docs/FOUNDATION.md`

## Key domains

- **Time**: clock in/out (optional `advisory` / enforce via settlement `enforce_flex_band`), breaks, shifts, templates, **work calendars** (Tagesmodell, Jahresperiode, Umschaltplan, MA-Zuordnung — `docs/TIME_MODEL.md`), timesheets (`expected_minutes`, `balance_minutes`, `evaluation_json`), Zeitkonten on approve, Monatsabschluss; `POST /time/timesheets/rebuild`; calendar/assignment changes auto-rebuild (12 weeks on new MA-Zuordnung); see `docs/FOUNDATION.md`; server Zeit tab: `apps/server/src/components/{WorkCalendar,ShiftWeek,Timesheets,TimeSettlement}Card.svelte`; client: `ClientTimePillar.svelte`, `ClientApprovalsPillar.svelte`
- **Absence**: requests, approve/reject/cancel, conflict pre-check; bulk `approve-pending`; `?status=` filter
- **Access**: zones, doors, badges, rules, occupancy, simulate-scan
- **Admin**: users, employees (active_to), audit, dashboard, policy
