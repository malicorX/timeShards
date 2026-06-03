# Project status (time foundation v1)

**Last updated:** 2026-06-03 · **Release:** [`v0.2.2`](docs/releases/v0.2.2.md) (app/API version **0.2.0**; tag notes for payroll bundle)

## Shipped

Germany-first **time tracking + access control** (Tauri + Axum + SQLite):

- **Sollzeit** from Arbeitskalender + Tagesmodell (not shift templates)
- Stempeln, breaks, weekly timesheets with Ist/Soll/Saldo and Tagesdetails
- Flex/overtime accounts on approve, month close, Lohn-CSV + Abwesenheiten-CSV (+ **Monats-Paket** in UI)
- Access simulation, anti-passback, occupancy
- Admin **Go-Live-Assistent**, production checklist, **Ersteinrichtung** hint (prod, &lt;2 MA), Zeit↔Zutritt KPIs
- **Perioden UI** (tabs, Jahresübersicht, Feiertage, Umschaltplan), shared UI tokens, pick-list Personal/Zutritt
- Hardware bridge helpers: `npm run verify:doors`, [HARDWARE.md](docs/HARDWARE.md) pilot checklist

## Verify locally

```powershell
npm run check:all
npm run verify:all          # check + foundation tests
npm run smoke:api           # full API smoke + door mapping
npm run smoke:production    # demo off, block default passwords
```

## Docs

| Doc | Purpose |
|-----|---------|
| [docs/FOUNDATION.md](docs/FOUNDATION.md) | What is implemented |
| [docs/PRODUCTION.md](docs/PRODUCTION.md) | Go-live without demo |
| [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) | Run apps |
| [CHANGELOG.md](CHANGELOG.md) | Version history |
| [docs/PILOT.md](docs/PILOT.md) | First customer cutover |
| [docs/PHASE2.md](docs/PHASE2.md) | Post-v1 (DATEV, hardware, …) |

## Not in v1

Native DATEV import, OEM Wiegand/OSDP in-process, automatic stamp↔building sync, Postgres/SaaS (M6).

**Roadmap M1–M5:** implementation complete in repo; remaining work is **pilot** (physical door bridge, payroll bureau CSV feedback).

**Planning:** [ROADMAP.md](ROADMAP.md) (active milestones M1–M6) · [docs/PHASE2.md](docs/PHASE2.md) (integration tracks) · [docs/UI_UX_GUIDE.md](docs/UI_UX_GUIDE.md) (UI refactor).
