# DATEV handoff (planned)

v1 ships **semicolon CSV** exports only — see [PAYROLL_EXPORT.md](./PAYROLL_EXPORT.md). Native DATEV import is **Phase 2** ([PHASE2.md](./PHASE2.md)).

## Before implementation

Collect from payroll bureau or customer:

1. **Product** — e.g. LODAS, Lohn und Gehalt, DATEV Lohn online (different import paths).
2. **Sample import file** — one successful month with anonymised data.
3. **Field mapping** — which columns map to Lohnarten, Kostenträger, SV-Tage, etc.

## Current CSV → typical mapping (draft)

| Our column (Lohn-CSV) | Typical DATEV use |
|----------------------|-------------------|
| `personal_nr` | Personalnummer |
| `ist_h` / `soll_h` / `saldo_h` | Bewegungsdaten / Zeitwirtschaft (product-specific) |
| `gleitzeit_konto_min` | Kontostand Gleitzeit (if supported) |
| `ueberstunden_konto_min` | Kontostand Überstunden |

| Our column (Abwesenheiten-CSV) | Typical DATEV use |
|-------------------------------|-------------------|
| `typ` | Abwesenheitsart (Urlaub/Krank/…) |
| `von` / `bis` | Zeitraum |

Do **not** treat this table as authoritative until validated against a real import spec.

## Implementation sketch (when spec exists)

1. Add `format=datev` (or separate route) behind feature flag.
2. Map approved timesheets + absences to target column layout.
3. Golden-file test against customer sample (checksum or row-by-row).
4. Document in release notes; keep CSV as fallback.

## Verify interim exports

```powershell
npm run verify:all
```
