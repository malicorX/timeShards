# Contributing

## Before you push

```powershell
npm install
npm run verify:all    # cargo check + svelte-check + timeshards-db tests + API smoke
```

Optional:

```powershell
npm run smoke:production   # TIMESHARDS_DISABLE_DEMO, blocked default passwords
```

## Layout

See [AGENTS.md](AGENTS.md) and [STATUS.md](STATUS.md).

## Commits

- Focused commits with clear messages (English or German).
- Match existing German UI strings for user-facing text.
- PowerShell scripts: use `;` not `&&`.

## Docs

- Time model: [docs/TIME_MODEL.md](docs/TIME_MODEL.md), [docs/FOUNDATION.md](docs/FOUNDATION.md)
- Production: [docs/PRODUCTION.md](docs/PRODUCTION.md)
