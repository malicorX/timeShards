# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Added

- Server **Ersteinrichtung** card on Übersicht when demo is off and fewer than 2 active employees.
- [docs/DATEV.md](docs/DATEV.md) — DATEV handoff prerequisites and draft column mapping.
- `scripts/_smoke-api.ps1` — shared stop/build/wait helpers for headless smoke.

### Fixed

- CI smoke: pre-build API, stop stray processes, wait for correct health (hw adapter / demo mode); longer timeout on GitHub Actions.
- Smoke scripts stop stray `timeshards-api` and wait for the correct `demo_seeding` flag (fixes verify after `smoke:production`).

### Changed

- [docs/PILOT.md](docs/PILOT.md) — one-page first customer cutover (shipped in prior commit).
- README and doc index point to v0.2.2 release notes.

## [0.2.2] — 2026-06-02

### Added

- `GET /api/v1/reports/absences/export` — approved absences overlapping month (payroll handoff CSV).
- Server **Monats-Paket** button (Lohn- + Abwesenheiten-CSV); Go-Live wizard step **Lohn-Export**.
- [docs/PHASE2.md](docs/PHASE2.md) — post-v1 roadmap tracks.

### Changed

- Smoke creates and approves an in-month absence before testing absences payroll export.
- [FOUNDATION_CHECKLIST.md](docs/FOUNDATION_CHECKLIST.md), [PRODUCTION.md](docs/PRODUCTION.md), [GETTING_STARTED.md](docs/GETTING_STARTED.md) — payroll handoff steps.

## [0.2.1] — 2026-06-02

### Added

- [STATUS.md](STATUS.md) — v1 product scope vs long-term ROADMAP.
- [CONTRIBUTING.md](CONTRIBUTING.md), [docs/PAYROLL_EXPORT.md](docs/PAYROLL_EXPORT.md).
- `npm run verify:all` — `check:all` + `verify:foundation` in one command.

### Changed

- Crate and app versions aligned to **0.2.0** (API health, Tauri bundles).
- CI: `verify-foundation` job runs DB tests + API smoke (no duplicate DB test job on rust).
- `start_all.ps1` prints Zeitbasis KPIs when the API comes up.
- Lohn-CSV export prefixed with UTF-8 BOM for Excel (DE).

### Fixed

- Server and client form label associations (svelte-check a11y clean).

## [0.2.0] — 2026-06-02

### Added

- Server admin split into tab components (`OverviewTab`, `PersonnelTab`, `AbsenceTab`, `AccessTab`, `SystemTab`).
- **Go-Live-Assistent** and production checklist on Übersicht.
- Dashboard KPIs for **Zeit ↔ Zutritt** (clocked-in vs. in building).
- Timesheet HTML export with **Tagesdetails** from weekly evaluation.
- Client app split into pillars (Zeit, Freigaben, Abwesenheit, Zutritt, Konto) plus login/settings shell.
- `docs/PRODUCTION.md`, `npm run foundation:health`, extended smoke assertions.

### Changed

- Work calendar UI: fill current and next calendar year in one action.
- Payroll CSV export uses Europe/Berlin month boundaries.

## [0.1.0] — initial

- TimeShards time foundation: work calendars, Tagesmodelle, Soll/Ist rebuild, timesheets, flex/overtime accounts, month close.
- Access control (simulated badges, anti-passback), Tauri server + client, Axum API on port 47821.
- Demo seed, migrations, `npm run verify:foundation`, CI smoke jobs.
