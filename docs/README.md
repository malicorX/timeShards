# AI TimeShards — documentation

| Document | Description |
|----------|-------------|
| [GETTING_STARTED.md](./GETTING_STARTED.md) | Install, run, first login, UI tour, LAN setup, troubleshooting |
| [API.md](./API.md) | REST API reference |
| [TIME_MODEL.md](./TIME_MODEL.md) | Work calendars, Tagesmodelle, evaluation (Soll/Ist) |
| [FOUNDATION.md](./FOUNDATION.md) | What is implemented vs planned for the time base |
| [FOUNDATION_CHECKLIST.md](./FOUNDATION_CHECKLIST.md) | Admin go-live checklist (Kalender, Soll, repair) |
| [PRODUCTION.md](./PRODUCTION.md) | Production deploy (env vars, Go-Live, verify) |
| [PAYROLL_EXPORT.md](./PAYROLL_EXPORT.md) | Lohn-CSV columns and parameters |
| [releases/v0.2.2.md](./releases/v0.2.2.md) | Latest release notes (payroll month bundle) |
| [releases/v0.2.0.md](./releases/v0.2.0.md) | Release notes for tag `v0.2.0` |
| [openapi.json](./openapi.json) | OpenAPI 3.0 subset (`GET /api/v1/openapi.json` when server runs) |
| [HARDWARE.md](./HARDWARE.md) | Simulator vs real readers, adapter rules |
| [../STATUS.md](../STATUS.md) | What ships today (v1 foundation summary) |
| [PHASE2.md](./PHASE2.md) | Post-v1 tracks (DATEV, hardware, …) |
| [../README.md](../README.md) | Project overview |
| [../ROADMAP.md](../ROADMAP.md) | Product direction (high level) |
| [../ROADMAP_DETAILS.md](../ROADMAP_DETAILS.md) | Detailed vision and modules |

PrimeWeb reference PDFs (`docs/pWA_*.pdf`, `pWM_*.pdf`, `pWT_*.pdf`) are **not in git** — keep them locally for design reference only.

## Helper scripts (Windows)

From the repository root:

```powershell
.\scripts\start_all.ps1       # server (new window) + wait for API + client
.\scripts\start_server.ps1    # API + admin app only
.\scripts\start_client.ps1     # employee app (server must be running)
.\scripts\open-firewall.ps1   # LAN access (run as Administrator, once)
.\scripts\check.ps1           # cargo check + svelte-check (server & client)
.\scripts\build.ps1           # Tauri release builds (server + client)
.\scripts\smoke-test.ps1      # API health + login (server must run)
.\scripts\smoke-with-api.ps1  # fresh DB + headless API + full smoke
.\scripts\run-api.ps1         # headless API only
.\scripts\send-hw-tcp.ps1     # send one line to external TCP ingest (TIMESHARDS_HW_TCP_ADDR)
```

Or: `npm run check:all` · `npm run verify:foundation` · `npm run smoke` · `npm run smoke:api` · `npm run smoke:production` · `npm run smoke:strict` · `npm run smoke:hw-external` · `npm run api`
