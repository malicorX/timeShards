# Lohn-CSV export

Interim payroll handoff before a full DATEV bridge. API: `GET /api/v1/reports/payroll/export`.

## Parameters

| Query | Required | Description |
|-------|----------|-------------|
| `year` | yes | Calendar year (e.g. `2026`) |
| `month` | yes | Month `1`–`12` |
| `format` | no | `csv` (default) |
| `aggregate` | no | `employee` = one row per MA (month totals); omit = one row per approved calendar week |
| `employee_id` | no | Restrict to one employee (managers/HR); employees only see self |

Month boundaries use **Europe/Berlin** (approved weeks whose `period_start` falls in that month).

## CSV columns (semicolon-separated)

| Column | Meaning |
|--------|---------|
| `personal_nr` | Employee number |
| `name` | Display name |
| `jahr` / `monat` | Export month |
| `kw_beginn` | Week `period_start` (ISO); empty when `aggregate=employee` |
| `ist_min` / `ist_h` | Worked minutes / hours (decimal) |
| `soll_min` / `soll_h` | Expected (Soll) minutes / hours |
| `saldo_min` / `saldo_h` | Balance minutes / hours |
| `gutschrift_min` | Credited minutes from approved week evaluation |
| `ueberstunden_min` | Overtime minutes on timesheet |
| `gleitzeit_konto_min` | Current flex account balance |
| `ueberstunden_konto_min` | Current overtime account balance |

File starts with a **UTF-8 BOM** so Excel (DE) opens encoding and `;` correctly.

Only **approved** timesheets are included. If none: a single placeholder line `(keine freigegebenen Wochen im Monat)`.

## UI

Server → **Zeit** → **Abschluss & Export** → **Lohn-CSV herunterladen**.

## Verify

```powershell
npm run smoke:api   # includes payroll CSV header check
```

## Not included (later)

- DATEV LODAS / Lohn und Gehalt native import format
- Lohnarten mapping, Kostenträger, Sozialversicherung

See [PRODUCTION.md](./PRODUCTION.md) step 5 and [FOUNDATION.md](./FOUNDATION.md).
