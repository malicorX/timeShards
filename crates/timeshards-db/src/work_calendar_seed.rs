//! Idempotent seed for default workday models, holiday calendar, and work calendar days.

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::SqlitePool;
use tracing::info;
use uuid::Uuid;

use crate::work_model::{standard_models, SettlementRuleConfig, WorkdayModelConfig};
use crate::work_rotation::DEFAULT_ROTATION_PLAN_ID;

pub const DEFAULT_WORK_CALENDAR_ID: &str = "wc-default-standard";
pub const DEFAULT_HOLIDAY_CALENDAR_ID: &str = "hc-de-standard";
pub const DEFAULT_SETTLEMENT_RULE_ID: &str = "sr-weekly-default";

/// Ensures built-in models, DE holiday calendar, standard Mo–Fr calendar, and employee assignments.
pub async fn ensure_work_calendar_foundation(pool: &SqlitePool) -> anyhow::Result<()> {
    seed_workday_models(pool).await?;
    seed_holiday_calendar(pool).await?;
    seed_work_calendar(pool).await?;
    seed_settlement_rules(pool).await?;
    seed_rotation_plans(pool).await?;
    let year = Utc::now().year();
    generate_work_calendar_year(pool, DEFAULT_WORK_CALENDAR_ID, year).await?;
    generate_work_calendar_year(pool, DEFAULT_WORK_CALENDAR_ID, year + 1).await?;
    assign_all_active_employees(pool).await?;
    crate::timesheet_compute::ensure_current_week_draft_timesheets(pool).await?;
    Ok(())
}

async fn seed_settlement_rules(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settlement_rules WHERE id = ?")
        .bind(DEFAULT_SETTLEMENT_RULE_ID)
        .fetch_one(pool)
        .await?;
    let config = serde_json::to_string(&SettlementRuleConfig::default())?;
    if exists == 0 {
        sqlx::query(
            r#"
            INSERT INTO settlement_rules (id, name, period_kind, config_json, created_at, updated_at)
            VALUES (?, 'Wochenabschluss Standard', 'week', ?, ?, ?)
            "#,
        )
        .bind(DEFAULT_SETTLEMENT_RULE_ID)
        .bind(&config)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
        info!("default settlement rule seeded");
    }
    sqlx::query(
        "UPDATE work_calendars SET settlement_rule_id = ? WHERE id = ? AND settlement_rule_id IS NULL",
    )
    .bind(DEFAULT_SETTLEMENT_RULE_ID)
    .bind(DEFAULT_WORK_CALENDAR_ID)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_workday_models(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    for (id, name, config) in standard_models() {
        let json = serde_json::to_string(&config)?;
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workday_models WHERE id = ?")
                .bind(id)
                .fetch_one(pool)
                .await?;
        if exists > 0 {
            sqlx::query(
                "UPDATE workday_models SET name = ?, config_json = ?, updated_at = ? WHERE id = ?",
            )
            .bind(name)
            .bind(&json)
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO workday_models (id, name, description, config_json, created_at, updated_at)
                VALUES (?, ?, NULL, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(name)
            .bind(&json)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
            info!(id, name, "workday model seeded");
        }
    }
    Ok(())
}

async fn seed_holiday_calendar(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    let year = Utc::now().year();
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM holiday_calendars WHERE id = ?",
    )
    .bind(DEFAULT_HOLIDAY_CALENDAR_ID)
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        sqlx::query(
            r#"
            INSERT INTO holiday_calendars (id, name, region_code, year_from, year_to, created_at)
            VALUES (?, 'Deutschland (Basis)', 'DE', ?, ?, ?)
            "#,
        )
        .bind(DEFAULT_HOLIDAY_CALENDAR_ID)
        .bind(year - 1)
        .bind(year + 2)
        .bind(&now)
        .execute(pool)
        .await?;
        info!("holiday calendar seeded");
    }

    for (date, name) in german_public_holidays(year - 1)
        .into_iter()
        .chain(german_public_holidays(year))
        .chain(german_public_holidays(year + 1))
        .chain(german_public_holidays(year + 2))
    {
        let day_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM holiday_calendar_days WHERE calendar_id = ? AND date = ?",
        )
        .bind(DEFAULT_HOLIDAY_CALENDAR_ID)
        .bind(&date)
        .fetch_one(pool)
        .await?;
        if day_exists > 0 {
            continue;
        }
        sqlx::query(
            r#"
            INSERT INTO holiday_calendar_days (calendar_id, date, day_kind, name, workday_model_id)
            VALUES (?, ?, 'holiday', ?, 'wm-holiday-paid')
            "#,
        )
        .bind(DEFAULT_HOLIDAY_CALENDAR_ID)
        .bind(&date)
        .bind(name)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn german_public_holidays(year: i32) -> Vec<(String, &'static str)> {
    vec![
        (format!("{year}-01-01"), "Neujahr"),
        (format!("{year}-05-01"), "Tag der Arbeit"),
        (format!("{year}-10-03"), "Tag der Deutschen Einheit"),
        (format!("{year}-12-25"), "1. Weihnachtstag"),
        (format!("{year}-12-26"), "2. Weihnachtstag"),
    ]
}

async fn seed_work_calendar(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_calendars WHERE id = ?")
        .bind(DEFAULT_WORK_CALENDAR_ID)
        .fetch_one(pool)
        .await?;
    if exists > 0 {
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO work_calendars (id, name, holiday_calendar_id, week_close_weekday, created_at, updated_at)
        VALUES (?, 'Standard Büro (Mo–Fr 8h)', ?, 6, ?, ?)
        "#,
    )
    .bind(DEFAULT_WORK_CALENDAR_ID)
    .bind(DEFAULT_HOLIDAY_CALENDAR_ID)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    info!("default work calendar seeded");
    Ok(())
}

/// Fill `work_calendar_days` for a calendar year (Mon–Fri std-8h, weekends rest).
pub async fn generate_work_calendar_year(
    pool: &SqlitePool,
    calendar_id: &str,
    year: i32,
) -> anyhow::Result<u32> {
    let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
    let mut inserted = 0u32;
    let mut day = start;
    while day <= end {
        let date_str = day.format("%Y-%m-%d").to_string();
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM work_calendar_days WHERE calendar_id = ? AND date = ?",
        )
        .bind(calendar_id)
        .bind(&date_str)
        .fetch_one(pool)
        .await?;
        if exists == 0 {
            let wd = day.weekday().num_days_from_monday();
            let model_id = if wd < 5 { "wm-std-8h" } else { "wm-rest" };
            sqlx::query(
                "INSERT INTO work_calendar_days (calendar_id, date, workday_model_id) VALUES (?, ?, ?)",
            )
            .bind(calendar_id)
            .bind(&date_str)
            .bind(model_id)
            .execute(pool)
            .await?;
            inserted += 1;
        }
        day += chrono::Duration::days(1);
    }
    if inserted > 0 {
        info!(calendar_id, year, inserted, "work calendar days generated");
    }
    Ok(inserted)
}

/// Assign default work calendar when employee has none. Returns true if a row was inserted.
pub async fn grant_default_work_calendar(
    pool: &SqlitePool,
    employee_id: &str,
) -> anyhow::Result<bool> {
    let has: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employee_work_assignments WHERE employee_id = ?",
    )
    .bind(employee_id)
    .fetch_one(pool)
    .await?;
    if has > 0 {
        return Ok(false);
    }

    let valid_from = format!("{}-01-01", Utc::now().year() - 1);
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO employee_work_assignments (
            id, employee_id, work_calendar_id, valid_from, valid_to, part_time_percent, notes, created_at
        ) VALUES (?, ?, ?, ?, NULL, 100, 'Standard-Kalender (automatisch)', ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(employee_id)
    .bind(DEFAULT_WORK_CALENDAR_ID)
    .bind(&valid_from)
    .bind(&now)
    .execute(pool)
    .await?;

    let _ = crate::timesheet_compute::rebuild_timesheets_for_employee_recent(
        pool,
        employee_id,
        crate::timesheet_compute::REBUILD_WEEKS_COPY_OR_ASSIGN,
    )
    .await?;

    Ok(true)
}

/// Assign default calendar to every active employee who has none. Returns count newly assigned.
pub async fn assign_all_active_without_work_calendar(pool: &SqlitePool) -> anyhow::Result<u32> {
    let employees: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM employees WHERE active_to IS NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut assigned = 0u32;
    for emp_id in employees {
        if grant_default_work_calendar(pool, &emp_id).await? {
            assigned += 1;
        }
    }
    Ok(assigned)
}

pub async fn assign_all_active_employees(pool: &SqlitePool) -> anyhow::Result<()> {
    let employees: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM employees WHERE active_to IS NULL",
    )
    .fetch_all(pool)
    .await?;

    let valid_from = format!("{}-01-01", Utc::now().year() - 1);
    let now = Utc::now().to_rfc3339();

    for emp_id in employees {
        let has: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM employee_work_assignments WHERE employee_id = ?",
        )
        .bind(&emp_id)
        .fetch_one(pool)
        .await?;
        if has > 0 {
            continue;
        }
        sqlx::query(
            r#"
            INSERT INTO employee_work_assignments (
                id, employee_id, work_calendar_id, valid_from, valid_to, part_time_percent, notes, created_at
            ) VALUES (?, ?, ?, ?, NULL, 100, 'Standard-Kalender (Seed)', ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&emp_id)
        .bind(DEFAULT_WORK_CALENDAR_ID)
        .bind(&valid_from)
        .bind(&now)
        .execute(pool)
        .await?;

        let _ = crate::timesheet_compute::rebuild_timesheets_for_employee_recent(
            pool,
            &emp_id,
            crate::timesheet_compute::REBUILD_WEEKS_COPY_OR_ASSIGN,
        )
        .await?;
    }
    Ok(())
}

/// Two-week rotation: week A 8h Mo–Fr, week B 6h Mo–Fr (demo Umschaltplan; not linked to default calendar).
async fn seed_rotation_plans(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_rotation_plans WHERE id = ?")
        .bind(DEFAULT_ROTATION_PLAN_ID)
        .fetch_one(pool)
        .await?;
    if exists == 0 {
        let anchor = format!("{}-01-05", Utc::now().year());
        sqlx::query(
            r#"
            INSERT INTO work_rotation_plans (id, name, anchor_date, cycle_days, created_at, updated_at)
            VALUES (?, '2-Wochen Wechsel 8h/6h', ?, 14, ?, ?)
            "#,
        )
        .bind(DEFAULT_ROTATION_PLAN_ID)
        .bind(&anchor)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
        let slots = [
            "wm-std-8h", "wm-std-8h", "wm-std-8h", "wm-std-8h", "wm-std-8h", "wm-rest", "wm-rest",
            "wm-short-6h", "wm-short-6h", "wm-short-6h", "wm-short-6h", "wm-short-6h", "wm-rest",
            "wm-rest",
        ];
        for (idx, model_id) in slots.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO work_rotation_slots (plan_id, slot_index, workday_model_id)
                VALUES (?, ?, ?)
                "#,
            )
            .bind(DEFAULT_ROTATION_PLAN_ID)
            .bind(idx as i32)
            .bind(model_id)
            .execute(pool)
            .await?;
        }
        info!("rotation plan seeded");
    }
    Ok(())
}

#[allow(dead_code)]
pub fn model_config_std_8h() -> WorkdayModelConfig {
    WorkdayModelConfig::default()
}
