# Contributing

## Before you push

```powershell
npm install
npm run ship          # full pilot gate (check + DB tests + API smoke + production smoke)
# or: npm run verify:all && npm run smoke:production
```

## Layout

See [AGENTS.md](AGENTS.md) and [STATUS.md](STATUS.md).

## Commits

- Focused commits with clear messages (English or German).
- Match existing German UI strings for user-facing text.
- PowerShell scripts: use `;` not `&&`.

## Docs

- Time model: [docs/TIME_MODEL.md](docs/TIME_MODEL.md), [docs/FOUNDATION.md](docs/FOUNDATION.md)
- Production: [docs/PRODUCTION.md](docs/PRODUCTION.md), pilot: [docs/PILOT.md](docs/PILOT.md)
