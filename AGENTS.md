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
npm run verify:all              # check:all + verify:foundation
npm run verify:pilot            # verify:all + smoke:production (go-live gate)
Headless smokes pick a free port when **47821** is already in use (e.g. Tauri server running).
npm run verify:doors            # door/reader table for HW bridge (API must run)
npm run foundation:health       # GET /health time_foundation (API must run)
```

Default logins: `admin` / `admin` (server, badge `DEMO-ADMIN-001`), `demo` / `demo` (client employee, `DEMO-0002`), `manager` / `demo` (approvals, `DEMO-0003`). Override initial admin via `TIMESHARDS_ADMIN_PASSWORD` on empty DB. Demo accounts, badges, and week data are ensured on each server/API start via `ensure_demo_accounts` / `seed_demo_week_data` unless `TIMESHARDS_DISABLE_DEMO=1`.

CI: `.github/workflows/ci.yml` runs `cargo check` and `svelte-check` on push/PR.

## Layout

| Path | Role |
|------|------|
| `crates/timeshards-api` | REST routes, access eval, ArbZG policy |
| `crates/timeshards-db` | Migrations, seed |
| `apps/server` | Admin Tauri app — tabs: `OverviewTab`, `PersonnelTab`, …; Übersicht: `SetupGuideCard` (prod, &lt;2 MA), `ProductionChecklistCard`, `ProductionWizard`; Zeit: `WorkCalendarCard`, …, `TimeSettlementCard` |
| `apps/client` | Employee Tauri app — pillars: `ClientTimePillar`, `ClientApprovalsPillar`, `ClientAbsencePillar`, `ClientAccessPillar`, `ClientAccountPillar`; shell: `ClientAppShell`, `ClientLoginView`, `ClientSettingsView` |
| `docs/GETTING_STARTED.md` | User guide |
| `docs/PRODUCTION.md` | Go-live without demo |
| `docs/API.md` | REST reference |
| `docs/TIME_MODEL.md` | Work calendar / Soll-Ist model |
| `docs/FOUNDATION.md` | Time foundation — implemented vs planned |
| `docs/FOUNDATION_CHECKLIST.md` | Admin go-live checklist for work calendar / Soll |
| `docs/openapi.json` | OpenAPI subset; served at `GET /api/v1/openapi.json` |

## Conventions

- PowerShell: use `;` not `&&`
- Minimal diffs; match existing German UI strings; UI work follows `docs/UI_UX_GUIDE.md` (shared tokens, informative + clickable patterns)
- No commits unless the user asks
- Access: simulated badges only (`DEMO-*` UIDs); anti-passback via in/out readers; `TIMESHARDS_HW_ADAPTER=sim|external`; REST `simulate-scan`, channel `hardware-present` (poll `GET /access/events?since=`), external TCP lines (credential + door + reader_offline); see `docs/HARDWARE.md`
- New employees/users often get Büro zone allow rule via `grant_default_zone_access`; existing MA: `POST /admin/employees/{id}/grant-zone-access`
- Work calendar (Sollzeit): `grant_work_calendar` on `POST /admin/employees` (default); `POST /admin/employees/{id}/grant-work-calendar`; bulk `POST /admin/foundation-fix`; week bounds `GET /time/calendar-week`; see `docs/FOUNDATION.md`

## Planning

- Active milestones: `ROADMAP.md` (not the micro-kernel appendix unless explicitly requested)
- UI work: `docs/UI_UX_GUIDE.md`

## Key domains

- **Time**: clock in/out (optional `advisory` / enforce via settlement `enforce_flex_band`), breaks, shifts, templates, **work calendars** (Tagesmodell, Jahresperiode, Umschaltplan, MA-Zuordnung — `docs/TIME_MODEL.md`), timesheets (`expected_minutes`, `balance_minutes`, `evaluation_json`), Zeitkonten on approve, Monatsabschluss; payroll handoff: `GET /reports/payroll/export`, `GET /reports/absences/export` (UTF-8 BOM CSV; UI **Monats-Paket** in `TimeSettlementCard`); `docs/PAYROLL_EXPORT.md`; `POST /time/timesheets/rebuild`; calendar/assignment changes auto-rebuild (12 weeks on new MA-Zuordnung); see `docs/FOUNDATION.md`; server Zeit tab: `apps/server/src/components/{WorkCalendar,ShiftWeek,Timesheets,TimeSettlement}Card.svelte`; client: `ClientTimePillar.svelte`, `ClientApprovalsPillar.svelte`
- **Absence**: requests, approve/reject/cancel, conflict pre-check; bulk `approve-pending`; `?status=` filter
- **Access**: zones, doors, badges, rules, occupancy, simulate-scan
- **Admin**: users, employees (active_to), audit, dashboard, policy
