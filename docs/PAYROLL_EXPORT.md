# Lohn- & Abwesenheits-CSV

Interim payroll handoff before a full DATEV bridge.

| Export | API |
|--------|-----|
| Stunden (freigegebene KW) | `GET /api/v1/reports/payroll/export` |
| Abwesenheiten (freigegeben) | `GET /api/v1/reports/absences/export` |

## Lohn-CSV (`/reports/payroll/export`)

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

## Abwesenheiten-CSV (`/reports/absences/export`)

| Query | Required | Description |
|-------|----------|-------------|
| `year`, `month` | yes | Calendar month (Berlin) |
| `format` | no | `csv` |
| `employee_id` | no | Optional filter |

Includes **approved** absence requests that overlap the month (`starts_at` / `ends_at`). Columns:

`personal_nr;name;jahr;monat;typ;von;bis;grund`

UTF-8 BOM for Excel. Placeholder row if none in month.

## UI

Server → **Zeit** → **Abschluss & Export**:

- **Monats-Paket (beide CSV)** — downloads Lohn- then Abwesenheiten-CSV for the selected month
- **Lohn-CSV** / **Abwesenheiten-CSV** — individual files

Go-Live-Assistent includes a **Lohn-Export** step linking here.

## Verify

```powershell
npm run smoke:api   # includes payroll CSV header check
```

## Not included (later)

- DATEV LODAS / Lohn und Gehalt native import format
- Lohnarten mapping, Kostenträger, Sozialversicherung

See [PRODUCTION.md](./PRODUCTION.md) step 5 and [FOUNDATION.md](./FOUNDATION.md).
