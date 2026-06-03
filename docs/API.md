# AI TimeShards — REST API

Base URL: `http://<host>:47821` (default port **47821**).

Authentication: `Authorization: Bearer <token>` from `POST /api/v1/auth/login`.

Errors return JSON with a `message` field and appropriate HTTP status.

## OpenAPI

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/openapi.json` | OpenAPI 3.0.3 document (no auth); also in [openapi.json](openapi.json) |

## Health & auth

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health` | `{ …, hardware_adapter, time_foundation? { workday_models, work_calendars, active_employees, employees_without_work_calendar, current_week_drafts_without_soll } }` — no auth |
| POST | `/api/v1/auth/login` | Body: `{ username, password }` → token + user (incl. `employee_id` / `employee_no` when linked). Returns **403** for built-in default passwords when `TIMESHARDS_DISABLE_DEMO` or `TIMESHARDS_BLOCK_DEFAULT_PASSWORDS` is set |
| GET | `/api/v1/auth/me` | Current user summary (same fields as login user) |
| GET | `/api/v1/me/work-summary` | Clock status, `work_calendar_assigned`, `flex_balance_minutes`, `current_week` (Ist/Soll/Saldo + `work_calendar_name`; auto-rebuild draft if assigned), `draft_timesheets` (own), `my_pending_absences`, `team_draft_timesheets` (managers), `pending_*` (approvers) |
| POST | `/api/v1/auth/logout` | Invalidate session |
| POST | `/api/v1/auth/change-password` | Body: `{ current_password, new_password }` |

## Admin

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/admin/dashboard` | KPIs (incl. `draft_timesheets`, `planned_shifts_this_week`, `employees_without_work_calendar`, `timesheets_current_week_no_soll`, `time_access_mismatch_count`, `time_access_mismatches[]`), door alerts, occupancy, `demo_seeding_enabled`, `default_password_login_blocked`, `hardware_adapter` |
| POST | `/api/v1/admin/foundation-fix` | Assign missing default work calendars + rebuild current calendar week for all employees |
| POST | `/api/v1/admin/employees/{id}/grant-work-calendar` | Standard-Arbeitskalender wenn noch keine Zuordnung |
| GET | `/api/v1/admin/sites` | Sites (name, timezone) |
| GET | `/api/v1/admin/users` | List users |
| POST | `/api/v1/admin/users` | Create user + linked employee |
| GET | `/api/v1/admin/employees` | List employees (`work_calendar_assigned` per row) |
| POST | `/api/v1/admin/employees` | Create employee; optional `issue_badge`, `grant_zone_access`, `grant_work_calendar` (default true), auto `employee_no` |
| PATCH | `/api/v1/admin/employees/{id}` | Update name/org; `user_id` or `""` to unlink login |
| POST | `/api/v1/admin/employees/{id}/deactivate` | Set `active_to`, revoke badges |
| POST | `/api/v1/admin/users/{id}/disable` | Disable login, end sessions |
| POST | `/api/v1/admin/users/{id}/enable` | Re-enable user |
| POST | `/api/v1/admin/users/{id}/reset-password` | Body: `{ new_password }`, clears sessions |
| GET | `/api/v1/admin/users?include_inactive=true` | Include disabled users |
| POST | `/api/v1/admin/employees/{id}/reactivate` | Clear `active_to` |
| POST | `/api/v1/admin/employees/{id}/grant-zone-access` | Allow rule on Büro (or first zone) |
| GET | `/api/v1/admin/employees?include_inactive=true` | Include deactivated employees |
| GET | `/api/v1/admin/employees?q=` | Search by name, PN, username |
| GET | `/api/v1/admin/users?q=` | Search users |
| GET | `/api/v1/admin/audit` | `?object_type=&action=&actor_type=&limit=` (max 500) |
| GET | `/api/v1/admin/roles` | List roles |
| GET | `/api/v1/admin/sites` | List sites |

### Create employee body

```json
{
  "display_name": "Max Mustermann",
  "employee_no": "E0007",
  "org_unit": "Produktion",
  "user_id": null,
  "issue_badge": true,
  "grant_zone_access": true,
  "grant_work_calendar": true
}
```

Omit or empty `employee_no` for auto-numbering (`E0001`, …). With `issue_badge: true`, creates credential `DEMO-{employee_no}`. With `grant_zone_access: true` (default), adds allow rule on zone **Büro** (or first zone). With `grant_work_calendar: true` (default), assigns `wc-default-standard` when the employee has no calendar yet and rebuilds recent weeks. New users via `POST /admin/users` get the same zone rule automatically.

## Time

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/time/status` | Clock/break state for current employee |
| GET | `/api/v1/time/calendar-week` | Berlin KW bounds (`period_start`, `period_end`); optional `?at=` RFC3339 |
| POST | `/api/v1/time/clock-in` | Clock in |
| POST | `/api/v1/time/clock-out` | Clock out |
| POST | `/api/v1/time/break-start` | Start break |
| POST | `/api/v1/time/break-end` | End break |
| GET | `/api/v1/time/events` | Punch list (`?employee_id=`, `?limit=`). Without `employee_id`: employees see own rows; managers/HR/admin see recent events for all staff (includes `employee_no`, `employee_name`) |
| POST | `/api/v1/time/corrections` | Manual correction (manager/HR) |
| GET/POST | `/api/v1/time/shifts` | List / create; `?from=&to=&employee_id=&status=` |
| POST | `/api/v1/time/shifts/{id}/publish` | Publish shift |
| POST | `/api/v1/time/shifts/publish-planned` | Publish all `planned` in week (`?week_start=`) |
| POST | `/api/v1/time/shifts/{id}/cancel` | Cancel shift |
| GET | `/api/v1/admin/policy` | Active ArbZG-oriented limits (minutes) |
| GET | `/api/v1/time/shifts/conflicts` | Overlap pre-check |
| GET/POST | `/api/v1/time/shift-templates` | Recurring templates; GET `?employee_id=` |
| POST | `/api/v1/time/shift-templates/apply-week` | `?week_start=` ISO date; optional `?employee_id=` |
| POST | `/api/v1/time/shift-templates/{id}/deactivate` | Soft-delete template |
| GET/POST | `/api/v1/time/workday-models` | Tagesmodelle (Soll, Pausen, Feiertags-Gutschrift) |
| PUT | `/api/v1/time/workday-models/{id}` | Tagesmodell aktualisieren; `config` ändert → Stundenzettel-Neuberechnung betroffener Kalender |
| GET | `/api/v1/time/work-calendars` | Arbeitskalender (Jahresperiode) |
| GET | `/api/v1/time/work-calendars/{id}/days` | `?from=&to=` (YYYY-MM-DD) |
| POST | `/api/v1/time/work-calendars/{id}/generate-year` | Body: `{ year }` — Mo–Fr / Wochenende befüllen |
| PUT | `/api/v1/time/work-calendars/{id}/days/{date}` | Body: `{ workday_model_id }` — Einzeltag überschreiben |
| GET/POST | `/api/v1/time/employee-work-assignments` | MA ↔ Kalender; GET `?employee_id=` |
| GET | `/api/v1/time/settlement-rules` | Abrechnungsregeln inkl. `config` (`enforce_flex_band`, `warn_negative_balance`) |
| PUT | `/api/v1/time/settlement-rules/{id}` | Body: `{ config }` — z. B. Gleitzeit erzwingen |
| GET | `/api/v1/time/work-rotation-plans` | Umschaltpläne mit Slots |
| PUT | `/api/v1/time/work-calendars/{id}/rotation` | Body: `{ rotation_plan_id: string \| null }` |
| POST | `/api/v1/time/work-calendars/{id}/copy-days` | Body: `{ source_from, source_to, target_from }` (YYYY-MM-DD) |
| GET | `/api/v1/time/settlement-periods/preview` | `?year=&month=&employee_id=` — Monatsvorschau |
| POST | `/api/v1/time/settlement-periods` | Body: `{ employee_id, year, month }` — Monatsabschluss (+ Konten-Ausgleich) |
| GET | `/api/v1/time/settlement-periods` | Abgeschlossene Monate (`?year=`, `?month=`, `?employee_id=`) |
| GET | `/api/v1/time/accounts` | Zeitkonten (`flex`, `overtime`); `?employee_id=` for managers |
| GET | `/api/v1/time/timesheets` | `?status=`, `?employee_id=`, `?period_start=` (managers); includes `expected_minutes`, `balance_minutes`, `evaluation` |
| POST | `/api/v1/time/timesheets/rebuild` | Recompute from punches; `?week_start=` ISO (Monday). Managers: all employees; employees: own profile only |
| POST | `/api/v1/time/timesheets/{id}/submit` | Submit for approval |
| POST | `/api/v1/time/timesheets/submit-drafts` | Bulk submit draft/rejected; `?period_start=` optional |
| POST | `/api/v1/time/timesheets/{id}/approve` | Approve |
| POST | `/api/v1/time/timesheets/approve-pending` | Approve all pending timesheets |
| POST | `/api/v1/time/timesheets/{id}/reject` | Body: `{ reason }` |
| GET | `/api/v1/time/clocked-in` | Employees currently clocked in (last punch is in/break) |

## Absence

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/absences` | List / create; `?status=` and `?employee_id=` (managers) |
| GET | `/api/v1/absences/conflicts` | Overlap pre-check |
| POST | `/api/v1/absences/{id}/approve` | Approve; rebuilds affected timesheet weeks |
| POST | `/api/v1/absences/approve-pending` | Approve all pending absence requests |
| POST | `/api/v1/absences/{id}/reject` | Body: `{ note }` |
| POST | `/api/v1/absences/{id}/cancel` | Cancel own request |

## Access

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/access/zones` | Zones |
| GET/POST | `/api/v1/access/doors` | Doors (incl. `reader_in_id`, `reader_out_id` for simulate-scan) |
| POST | `/api/v1/access/doors/{id}/status` | `closed`, `open`, `forced_open`, `alarm` |
| GET | `/api/v1/access/events` | Recent events; `?decision=grant\|deny&since=<RFC3339>&employee_no=&limit=` (`since` for polling after hardware-present) |
| POST | `/api/v1/access/simulate-scan` | Body: `{ credential_uid, reader_id }` — processes one event (no duplicate hardware queue) |
| POST | `/api/v1/access/hardware-present` | Same body — queues on hardware channel (worker path); returns `{ queued, reader_id, credential_uid }` |
| GET/POST | `/api/v1/access/badges` | List / issue |
| POST | `/api/v1/access/badges/{id}/revoke` | Revoke badge |
| GET/POST | `/api/v1/access/rules` | Allow rules; POST may include `valid_from` / `valid_to` (ISO) |
| PATCH | `/api/v1/access/rules/{id}` | Update `schedule_json`, `valid_from`, `valid_to` (null clears end date) |
| DELETE | `/api/v1/access/rules/{id}` | Delete rule |
| GET | `/api/v1/access/occupancy` | Per-zone occupancy |
| GET | `/api/v1/access/me` | Own badges, recent events, configured `readers` for simulate-scan |
| POST | `/api/v1/access/me/simulate-scan` | Scan with own badge (single processing, same as admin simulate) |

## Reports

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/reports/timesheets/export` | `?format=csv\|html&status=&period_start=` — employees without manager/HR role only see their own rows |
| GET | `/api/v1/reports/payroll/export` | `?year=&month=&format=csv&aggregate=employee&employee_id=` — approved weeks in calendar month (Berlin); semicolon CSV for payroll |
| GET | `/api/v1/reports/access/export` | `?format=csv\|html&from=&to=&limit=` — without manager/HR/security role, only own events |

## Roles (default seed)

- `system_admin` — full access
- `hr_admin` — HR + time + reports
- `security_operator` — access control
- `manager` — approvals + read
- `employee` — self-service time/absence/access

See [GETTING_STARTED.md](./GETTING_STARTED.md) for how to run the apps and try features in the UI.
