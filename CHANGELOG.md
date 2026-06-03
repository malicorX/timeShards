# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Added

- `GET /api/v1/reports/absences/export` — approved absences overlapping month (payroll handoff CSV).

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

- PrimeWeb-inspired time foundation: work calendars, Tagesmodelle, Soll/Ist rebuild, timesheets, flex/overtime accounts, month close.
- Access control (simulated badges, anti-passback), Tauri server + client, Axum API on port 47821.
- Demo seed, migrations, `npm run verify:foundation`, CI smoke jobs.
