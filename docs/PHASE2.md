# Phase 2 — after v1 time foundation

v1 (work calendar → Soll, rebuild, approvals, Konten, go-live UX) is documented in [FOUNDATION.md](./FOUNDATION.md) and [STATUS.md](../STATUS.md).

## Planned tracks

| Track | Goal | Notes |
|-------|------|--------|
| **DATEV** | Native or semi-native handoff to Lohn | Today: [PAYROLL_EXPORT.md](./PAYROLL_EXPORT.md) (Lohn- + Abwesenheiten-CSV). Needs target DATEV product + sample import spec from customer. |
| **First-run setup** | Empty-DB onboarding (company, first MA, badges) | Today: seed + `TIMESHARDS_ADMIN_PASSWORD` + Go-Live-Assistent for production KPIs. |
| **Hardware** | Production reader TCP / Primion path | Today: `sim` + `external` adapter; see [HARDWARE.md](./HARDWARE.md). |
| **Stamp ↔ door** | Optional auto-sync or alerts | Today: dashboard KPI Zeit↔Zutritt; process clarification only. |
| **Calendar UX** | Full-year editor | Today: KW copy + Jahr befüllen on work calendar. |

## Suggested order

1. Pilot customer on v1 (demo off, Go-Live, payroll CSVs).
2. DATEV or payroll bureau feedback → column mapping.
3. Hardware on one door if access control is in scope.
4. PostgreSQL / multi-site only when second site needs central DB.

## Verify before each release

```powershell
npm run verify:all
npm run smoke:production
```
