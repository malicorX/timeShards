# Project status (time foundation v1)

**Last updated:** 2026-06-02 · **Release:** [`v0.2.0`](docs/releases/v0.2.0.md) (app/API version **0.2.0**)

## Shipped

Germany-first **time tracking + access control** (Tauri + Axum + SQLite):

- **Sollzeit** from Arbeitskalender + Tagesmodell (not shift templates)
- Stempeln, breaks, weekly timesheets with Ist/Soll/Saldo and Tagesdetails
- Flex/overtime accounts on approve, month close, Lohn-CSV + Abwesenheiten-CSV export
- Access simulation, anti-passback, occupancy
- Admin **Go-Live-Assistent**, production checklist, Zeit↔Zutritt KPIs

## Verify locally

```powershell
npm run check:all
npm run verify:foundation
npm run smoke:production
```

## Docs

| Doc | Purpose |
|-----|---------|
| [docs/FOUNDATION.md](docs/FOUNDATION.md) | What is implemented |
| [docs/PRODUCTION.md](docs/PRODUCTION.md) | Go-live without demo |
| [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) | Run apps |
| [CHANGELOG.md](CHANGELOG.md) | Version history |

## Not in v1

DATEV, full-year calendar editor, automatic stamp↔building sync.

`ROADMAP.md` describes the long-term micro-kernel vision; the runnable product is documented in **FOUNDATION.md**.
