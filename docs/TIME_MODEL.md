# Time model (work calendar foundation)

TimeShards layering for expected work time and timesheet evaluation. This is the **source of truth** for Sollzeit; `shift_templates` remain for planned shift instances (legacy UI) until fully migrated.

## Concepts

| TimeShards concept | Purpose |
|--------------------|---------|
| **Workday model** (`workday_models`) — Tagesperiode | Per-day rules: Soll minutes, flex/core windows, break expectations, holiday credit |
| **Holiday calendar** (`holiday_calendars` + `holiday_calendar_days`) | Fixed/special days (public holidays, company days) |
| **Work calendar** (`work_calendars` + `work_calendar_days`) — Jahresperiode | Maps each date to a workday model |
| **Employee work assignment** (`employee_work_assignments`) | Which calendar applies, validity range, part-time % |

Built-in seed IDs (idempotent on API start):

- `wm-std-8h`, `wm-rest`, `wm-holiday-paid`
- `hc-de-standard` — DE public holidays (simplified list)
- `wc-default-standard` — Mo–Fr 8h, weekends rest; year days generated for current year ±1

## Calendar weeks

Stundenzettel weeks use **Monday 00:00 Europe/Berlin** through the following Monday (stored as UTC instants). Server and desktop UI week pickers (`weekRangeContaining`) use the same Berlin boundary so `period_start` in API calls matches rebuild/evaluation.

## Evaluation flow

1. Resolve **assignment** active on the Monday of the week.
2. For each of the 7 days: holiday override → else `work_calendar_days` → workday model config.
3. Sum **punches** (`time_events`) into daily worked/break minutes (unchanged).
4. **Expected** = model `expected_minutes` × part-time %; **credit** on holidays with `auto_credit_expected`.
5. **Approved absence** on a work day (`urlaub`, `krank`, `sonder`) credits full daily Soll (no “missing punch” warning); shown as `day_kind: absence` in evaluation.
6. **Timesheet** stores `worked_minutes`, `expected_minutes`, `overtime_minutes`, and `evaluation_json` (per-day breakdown + warnings).
7. **Weekly settlement** preview in `evaluation.settlement`: worked/expected/credited/balance totals, `week_close_weekday`; optional warning on negative weekly balance (`settlement_rules`).
8. ArbZG policy pack still applies (daily max, break warnings, weekly caps).

Freigabe einer Abwesenheit löst **Stundenzettel-Neuberechnung** für alle betroffenen Kalenderwochen aus. Genehmigte Abwesenheit (`urlaub`/`krank`/`sonder`) gutgeschrieben; `unbezahlt` setzt Tages-Soll auf 0.

`timesheets.balance_minutes` = Wochensaldo (Summe Tages-Saldi).

Rebuild: `POST /api/v1/time/timesheets/rebuild` (uses calendar when assignment exists).

**Automatic rebuild:** `clock_out` and `break_end` rebuild the employee’s current calendar week (draft timesheet). Calendar edits (`generate-year`, day override, `copy-days`, rotation) rebuild assigned employees server-side.

**Work summary:** `GET /api/v1/me/work-summary` includes `work_calendar_assigned`, `current_week` (Ist/Soll/Saldo, calendar name). If assigned but no row for this KW yet, the API rebuilds the draft timesheet once.

## API (admin / HR)

- `GET/POST /api/v1/time/workday-models`
- `PUT /api/v1/time/workday-models/{id}` — update name/description/config; **config change** rebuilds all calendars using that model (recent weeks)
- `GET /api/v1/time/work-calendars`
- `GET /api/v1/time/work-calendars/{id}/days?from=&to=`
- `POST /api/v1/time/work-calendars/{id}/generate-year` — fill Mo–Fr / weekend pattern
- `PUT /api/v1/time/work-calendars/{id}/days/{date}` — override one day (`YYYY-MM-DD`, body: `{ workday_model_id }`)
- `GET/POST /api/v1/time/employee-work-assignments`

Permissions: `Shift` read/create/update (same as shift templates).

## Monthly close (Monatsperiode)

- `GET /api/v1/time/settlement-periods/preview?year=&month=&employee_id=` — sums **approved** weeks with `period_start` in that calendar month
- `POST /api/v1/time/settlement-periods` — `{ employee_id, year, month }` closes month (blocks if draft/pending weeks remain)
- `GET /api/v1/time/settlement-periods` — list closed periods

## Calendar week (API)

- `GET /api/v1/time/calendar-week` — `period_start` / `period_end` (Berlin Monday 00:00); optional `?at=` RFC3339

## Calendar tools

- `POST /api/v1/time/work-calendars/{id}/copy-days` — `{ source_from, source_to, target_from }` (YYYY-MM-DD)

## Workday model rounding

- Optional `worked_rounding_minutes` in model JSON (e.g. `15`) — applied per day during rebuild

## Rotation (Umschaltplan)

- `work_rotation_plans` + `work_rotation_slots` — cyclic `workday_model_id` from `anchor_date`
- `work_calendars.rotation_plan_id` — when set, overrides `work_calendar_days` (holidays still win)
- Seed: `rp-14day-alt` (week 8h / week 6h); not linked to default calendar unless assigned
- `GET /api/v1/time/work-rotation-plans`
- `PUT /api/v1/time/work-calendars/{id}/rotation` — `{ "rotation_plan_id": "…" | null }`

## Punch flex advisory

- `POST clock-in` / `clock-out` may return `{ advisory: "…" }` when:
  - no active **Arbeitskalender** assignment (`Kein Arbeitskalender zugewiesen…`), or
  - local time (Europe/Berlin) is outside the model `flex_band`
- Settlement rule `enforce_flex_band: true` in `config_json` rejects the punch with HTTP 400 (flex band only; missing calendar stays advisory)
- Rebuild without calendar: timesheet gets ArbZG warnings only; `evaluation` has no Soll breakdown

## Later phases

- Rich calendar editor (bulk UI beyond copy-week)
- Block punches outside flex band (optional policy)
