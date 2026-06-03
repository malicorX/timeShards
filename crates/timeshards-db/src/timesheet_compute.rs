//! Worked minutes from punches and calendar-based timesheet rebuild.

use chrono::{DateTime, Duration, Utc};
use chrono::Datelike;
use chrono_tz::Europe::Berlin;

/// Recent weeks to rebuild after a single calendar-day override.
pub const REBUILD_WEEKS_DAY_OVERRIDE: u32 = 4;
/// After rotation attach/detach or generate-year.
pub const REBUILD_WEEKS_CALENDAR_EDIT: u32 = 8;
/// After copy-days or new employee calendar assignment.
pub const REBUILD_WEEKS_COPY_OR_ASSIGN: u32 = 12;
/// After Tagesmodell config change (all calendars using that model).
pub const REBUILD_WEEKS_WORKDAY_MODEL: u32 = 8;
use sqlx::SqlitePool;

use crate::calendar_compute::{compute_week_for_employee, upsert_timesheet_from_computation};

/// Calendar week `[start, end)` as UTC instants: **Monday 00:00 Europe/Berlin** (Germany-first).
pub fn week_bounds_utc(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let local = now.with_timezone(&Berlin);
    let days_from_monday = local.weekday().num_days_from_monday() as i64;
    let monday = local.date_naive() - Duration::days(days_from_monday);
    let start = monday
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Berlin)
        .single()
        .expect("Berlin midnight")
        .with_timezone(&Utc);
    (start, start + Duration::days(7))
}

pub async fn compute_daily_worked_minutes(
    pool: &SqlitePool,
    employee_id: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> anyhow::Result<(Vec<i64>, Vec<i64>)> {
    let ps = period_start.to_rfc3339();
    let pe = period_end.to_rfc3339();
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT kind, occurred_at FROM time_events
        WHERE employee_id = ? AND occurred_at >= ? AND occurred_at < ?
        ORDER BY occurred_at ASC
        "#,
    )
    .bind(employee_id)
    .bind(&ps)
    .bind(&pe)
    .fetch_all(pool)
    .await?;

    let mut daily_work = vec![0i64; 7];
    let mut daily_break = vec![0i64; 7];
    let mut work_start: Option<DateTime<Utc>> = None;
    let mut break_start: Option<DateTime<Utc>> = None;

    for (kind, at) in rows {
        let ts = DateTime::parse_from_rfc3339(&at)?.with_timezone(&Utc);
        match kind.as_str() {
            "clock_in" => {
                break_start = None;
                work_start = Some(ts);
            }
            "break_start" => {
                if let Some(ws) = work_start.take() {
                    add_interval_to_daily(&mut daily_work, period_start, ws, ts);
                }
                break_start = Some(ts);
            }
            "break_end" => {
                if let Some(bs) = break_start.take() {
                    add_interval_to_daily(&mut daily_break, period_start, bs, ts);
                }
                work_start = Some(ts);
            }
            "clock_out" => {
                if let Some(ws) = work_start.take() {
                    add_interval_to_daily(&mut daily_work, period_start, ws, ts);
                }
                break_start = None;
            }
            _ => {}
        }
    }
    if let Some(ws) = work_start {
        add_interval_to_daily(&mut daily_work, period_start, ws, period_end);
    }
    Ok((daily_work, daily_break))
}

fn berlin_day_index(week_start: DateTime<Utc>, instant: DateTime<Utc>) -> Option<i64> {
    let monday = week_start.with_timezone(&Berlin).date_naive();
    let day = instant.with_timezone(&Berlin).date_naive();
    let idx = (day - monday).num_days();
    if (0..7).contains(&idx) { Some(idx) } else { None }
}

fn berlin_day_end_utc(week_start: DateTime<Utc>, day_index: i64) -> DateTime<Utc> {
    let next = week_start.with_timezone(&Berlin).date_naive() + Duration::days(day_index + 1);
    next.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Berlin)
        .single()
        .expect("Berlin midnight")
        .with_timezone(&Utc)
}

fn add_interval_to_daily(
    daily: &mut [i64],
    week_start: DateTime<Utc>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) {
    let mut cursor = start;
    while cursor < end {
        let Some(day_index) = berlin_day_index(week_start, cursor) else {
            break;
        };
        let day_end_dt = berlin_day_end_utc(week_start, day_index);
        let segment_end = end.min(day_end_dt);
        daily[day_index as usize] += (segment_end - cursor).num_minutes().max(0);
        cursor = segment_end;
    }
}

async fn rebuild_one(
    pool: &SqlitePool,
    employee_id: &str,
    period_start: DateTime<Utc>,
) -> anyhow::Result<(bool, Vec<String>)> {
    let period_end = period_start + Duration::days(7);
    let comp = compute_week_for_employee(pool, employee_id, period_start).await?;
    let warnings = comp.warnings.clone();
    let updated = upsert_timesheet_from_computation(
        pool,
        employee_id,
        period_start,
        period_end,
        &comp,
    )
    .await?;
    Ok((updated, warnings))
}

/// Rebuild timesheets for all active employees in a week. Returns (updated count, warnings).
pub async fn rebuild_timesheets_for_week(
    pool: &SqlitePool,
    period_start: DateTime<Utc>,
) -> anyhow::Result<(u32, Vec<String>)> {
    let employees: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, employee_no FROM employees WHERE active_to IS NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut updated = 0u32;
    let mut warnings: Vec<String> = Vec::new();

    for (emp_id, emp_no) in employees {
        let (did, emp_warnings) = rebuild_one(pool, &emp_id, period_start).await?;
        for w in emp_warnings {
            warnings.push(format!("{emp_no}: {w}"));
        }
        if did {
            updated += 1;
        }
    }

    Ok((updated, warnings))
}

/// Rebuild one employee's timesheet for a week. Returns (0 or 1 updated, warnings).
pub async fn rebuild_timesheet_for_employee_week(
    pool: &SqlitePool,
    employee_id: &str,
    period_start: DateTime<Utc>,
) -> anyhow::Result<(u32, Vec<String>)> {
    let emp_no: String = sqlx::query_scalar("SELECT employee_no FROM employees WHERE id = ?")
        .bind(employee_id)
        .fetch_one(pool)
        .await?;

    let (did, emp_warnings) = rebuild_one(pool, employee_id, period_start).await?;
    let mut warnings: Vec<String> = Vec::new();
    for w in emp_warnings {
        warnings.push(format!("{emp_no}: {w}"));
    }
    Ok((if did { 1 } else { 0 }, warnings))
}

/// Rebuild recent calendar weeks for one employee (e.g. after work-calendar assignment).
pub async fn rebuild_timesheets_for_employee_recent(
    pool: &SqlitePool,
    employee_id: &str,
    weeks_back: u32,
) -> anyhow::Result<(u32, Vec<String>)> {
    let (week_start, _) = week_bounds_utc(Utc::now());
    let mut updated = 0u32;
    let mut warnings = Vec::new();
    for w in 0..=weeks_back {
        let ps = week_start - Duration::days(7 * i64::from(w));
        let (n, wlist) = rebuild_timesheet_for_employee_week(pool, employee_id, ps).await?;
        updated += n;
        warnings.extend(wlist);
    }
    Ok((updated, warnings))
}

/// Rebuild all calendar weeks overlapping an absence interval (after approve).
pub async fn rebuild_timesheets_for_absence_range(
    pool: &SqlitePool,
    employee_id: &str,
    starts_at: &str,
    ends_at: &str,
) -> anyhow::Result<(u32, Vec<String>)> {
    let abs_start = DateTime::parse_from_rfc3339(starts_at)?.with_timezone(&Utc);
    let abs_end = DateTime::parse_from_rfc3339(ends_at)?.with_timezone(&Utc);
    let (mut week_start, _) = week_bounds_utc(abs_start);
    let mut updated = 0u32;
    let mut warnings = Vec::new();
    let abs_end_week = week_bounds_utc(abs_end).0;

    while week_start <= abs_end_week {
        let (n, w) = rebuild_timesheet_for_employee_week(pool, employee_id, week_start).await?;
        updated += n;
        warnings.extend(w);
        week_start += Duration::days(7);
    }
    Ok((updated, warnings))
}

/// Rebuild recent weeks for all employees assigned to a work calendar (after calendar/rotation changes).
pub async fn rebuild_timesheets_for_calendar(
    pool: &SqlitePool,
    work_calendar_id: &str,
    weeks_back: u32,
) -> anyhow::Result<(u32, Vec<String>)> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let employees: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT employee_id FROM employee_work_assignments
        WHERE work_calendar_id = ?
          AND valid_from <= ?
          AND (valid_to IS NULL OR valid_to > ?)
        "#,
    )
    .bind(work_calendar_id)
    .bind(&today)
    .bind(&today)
    .fetch_all(pool)
    .await?;

    let (week_start, _) = week_bounds_utc(Utc::now());
    let mut updated = 0u32;
    let mut warnings = Vec::new();

    for emp_id in employees {
        for w in 0..=weeks_back {
            let ps = week_start - Duration::days(7 * i64::from(w));
            let (n, wlist) = rebuild_timesheet_for_employee_week(pool, &emp_id, ps).await?;
            updated += n;
            warnings.extend(wlist);
        }
    }
    Ok((updated, warnings))
}

/// Rebuild employees on all calendars that reference a workday model (days or rotation slots).
pub async fn rebuild_timesheets_for_workday_model(
    pool: &SqlitePool,
    workday_model_id: &str,
    weeks_back: u32,
) -> anyhow::Result<(u32, Vec<String>)> {
    let calendar_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT calendar_id FROM work_calendar_days WHERE workday_model_id = ?
        UNION
        SELECT DISTINCT wc.id FROM work_calendars wc
        JOIN work_rotation_slots wrs ON wrs.plan_id = wc.rotation_plan_id
        WHERE wrs.workday_model_id = ?
        "#,
    )
    .bind(workday_model_id)
    .bind(workday_model_id)
    .fetch_all(pool)
    .await?;

    let mut updated = 0u32;
    let mut warnings = Vec::new();
    for cal_id in calendar_ids {
        let (n, w) = rebuild_timesheets_for_calendar(pool, &cal_id, weeks_back).await?;
        updated += n;
        warnings.extend(w);
    }
    Ok((updated, warnings))
}

/// Employees with a calendar who need a current-week draft with Soll (missing row or draft/rejected with Soll=0).
async fn employees_needing_current_week_draft(pool: &SqlitePool) -> anyhow::Result<Vec<String>> {
    let (week_start, _) = week_bounds_utc(Utc::now());
    let ps = week_start.to_rfc3339();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT e.id FROM employees e
        WHERE e.active_to IS NULL
          AND EXISTS (
            SELECT 1 FROM employee_work_assignments a
            WHERE a.employee_id = e.id
              AND a.valid_from <= ?
              AND (a.valid_to IS NULL OR substr(a.valid_to, 1, 10) > ?)
          )
          AND NOT EXISTS (
            SELECT 1 FROM timesheets t
            WHERE t.employee_id = e.id
              AND t.period_start = ?
              AND t.status IN ('pending', 'approved')
          )
          AND (
            NOT EXISTS (
              SELECT 1 FROM timesheets t
              WHERE t.employee_id = e.id AND t.period_start = ?
            )
            OR EXISTS (
              SELECT 1 FROM timesheets t
              WHERE t.employee_id = e.id
                AND t.period_start = ?
                AND t.status IN ('draft', 'rejected')
                AND t.expected_minutes = 0
            )
          )
        "#,
    )
    .bind(&today)
    .bind(&today)
    .bind(&ps)
    .bind(&ps)
    .bind(&ps)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Count of current-week draft/rejected timesheets (or missing row) without Soll despite calendar.
pub async fn count_current_week_drafts_without_soll(pool: &SqlitePool) -> anyhow::Result<i64> {
    let employees = employees_needing_current_week_draft(pool).await?;
    Ok(employees.len() as i64)
}

/// Create or rebuild current-week drafts with calendar-backed Soll (startup / foundation-fix helper).
pub async fn ensure_current_week_draft_timesheets(pool: &SqlitePool) -> anyhow::Result<u32> {
    let (week_start, _) = week_bounds_utc(Utc::now());
    let employees = employees_needing_current_week_draft(pool).await?;
    let mut updated = 0u32;
    for emp_id in employees {
        let (n, _) = rebuild_timesheet_for_employee_week(pool, &emp_id, week_start).await?;
        updated += n;
    }
    if updated > 0 {
        tracing::info!(updated, "current-week draft timesheets ensured");
    }
    Ok(updated)
}

/// Alias for `ensure_current_week_draft_timesheets`.
pub async fn rebuild_stale_current_week_timesheets(pool: &SqlitePool) -> anyhow::Result<u32> {
    ensure_current_week_draft_timesheets(pool).await
}

/// Upsert draft timesheet for one employee/week from punches (with calendar + policy). Returns true if created/updated.
pub async fn upsert_draft_timesheet_for_week(
    pool: &SqlitePool,
    employee_id: &str,
    period_start: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let (updated, _) = rebuild_one(pool, employee_id, period_start).await?;
    Ok(updated)
}
