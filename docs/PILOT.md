# Pilot deployment (v1 time foundation)

One-page cutover for a first customer on **AI TimeShards** without demo data. Full detail: [PRODUCTION.md](./PRODUCTION.md), [FOUNDATION_CHECKLIST.md](./FOUNDATION_CHECKLIST.md).

## 1. Environment (server machine)

| Variable | Value |
|----------|--------|
| `TIMESHARDS_DISABLE_DEMO` | `1` |
| `TIMESHARDS_ADMIN_PASSWORD` | Strong password (empty DB only) |
| `TIMESHARDS_BLOCK_DEFAULT_PASSWORDS` | `1` (recommended) |

Optional: `TIMESHARDS_HW_ADAPTER=sim` until real readers are wired ([HARDWARE.md](./HARDWARE.md)).

## 2. First start

```powershell
cd m:\Data\Projects\ai_timeshards
$env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"
npm run start:pilot # Server + Client (recommended for desktop pilot)
# npm run api:pilot   # headless API only
```

- Log in as `admin` with your `TIMESHARDS_ADMIN_PASSWORD`.
- **Übersicht** → **Go-Live-Assistent** or **Produktions-Checkliste** until all items are green (or documented exceptions).

## 3. Master data

1. **Personal** — employees, users, badges; ensure **Arbeitskalender** on every active MA.
2. **Zeit** → Arbeitskalender — Tagesmodelle, Kalender, **Jahr befüllen** if needed.
3. **Zutritt** — zones, doors, rules (if access is in scope).

Run **Zeitbasis reparieren** on Übersicht if KPIs show MA ohne Kalender or KW ohne Soll.

## 4. Trial week

- Client: `demo`-style users clock in/out; verify KW shows **Ist · Soll · Saldo**.
- Manager: approve timesheets and absences.
- Check **Zeit ↔ Zutritt** on Übersicht (0 or documented Homeoffice cases).

## 5. Month-end (payroll handoff)

When all weeks in the month are **freigegeben**:

1. **Zeit** → **Abschluss & Export** → **Monatsabschluss** (if using Konten reconciliation).
2. **Monats-Paket (beide CSV)** or separate Lohn- / Abwesenheiten-CSV → [PAYROLL_EXPORT.md](./PAYROLL_EXPORT.md).

No DATEV import in v1 — hand CSVs to Lohnbüro / Excel.

## 6. Verify before go-live

```powershell
npm run pilot:ready
```

Same as `verify:pilot` plus a short go-live checklist on success. `verify:pilot` alone runs `check:all`, DB tests, full API smoke (incl. door mapping), and production-mode smoke (`TIMESHARDS_DISABLE_DEMO`, blocked default passwords).

With API running for a hardware pilot: `npm run verify:doors`

## After pilot

Collect feedback on CSV columns and processes → [PHASE2.md](./PHASE2.md) (DATEV, hardware, …).
