//! Workday model config change propagates to timesheet expected minutes via rebuild.

use chrono::{Datelike, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{
    ensure_work_calendar_foundation, parse_workday_config, rebuild_timesheet_for_employee_week,
    rebuild_timesheets_for_workday_model, week_bounds_utc, REBUILD_WEEKS_WORKDAY_MODEL,
    DEFAULT_WORK_CALENDAR_ID,
};

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("memory db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrate");
    pool
}

#[tokio::test]
async fn workday_model_config_change_updates_week_expected() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T99', 'Model MA', ?, ?)",
    )
    .bind(&emp_id)
    .bind(&valid_from)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO employee_work_assignments (
            id, employee_id, work_calendar_id, valid_from, valid_to, part_time_percent, notes, created_at
        ) VALUES (?, ?, ?, ?, NULL, 100, NULL, ?)
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&emp_id)
    .bind(DEFAULT_WORK_CALENDAR_ID)
    .bind(&valid_from)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let (week_start, _) = week_bounds_utc(Utc::now());
    let period_start = week_start.to_rfc3339();

    rebuild_timesheet_for_employee_week(&pool, &emp_id, week_start)
        .await
        .unwrap();

    let before: i64 = sqlx::query_scalar(
        "SELECT expected_minutes FROM timesheets WHERE employee_id = ? AND period_start = ?",
    )
    .bind(&emp_id)
    .bind(&period_start)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(before >= 5 * 8 * 60, "baseline Mo–Fr 8h Soll");

    let json: String = sqlx::query_scalar("SELECT config_json FROM workday_models WHERE id = 'wm-std-8h'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut cfg = parse_workday_config(&json).unwrap();
    cfg.expected_minutes = 6 * 60;
    let updated = serde_json::to_string(&cfg).unwrap();
    sqlx::query("UPDATE workday_models SET config_json = ? WHERE id = 'wm-std-8h'")
        .bind(&updated)
        .execute(&pool)
        .await
        .unwrap();

    let (n, _) =
        rebuild_timesheets_for_workday_model(&pool, "wm-std-8h", REBUILD_WEEKS_WORKDAY_MODEL)
            .await
            .unwrap();
    assert!(n >= 1, "at least one timesheet row updated");

    let after: i64 = sqlx::query_scalar(
        "SELECT expected_minutes FROM timesheets WHERE employee_id = ? AND period_start = ?",
    )
    .bind(&emp_id)
    .bind(&period_start)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        after < before,
        "6h model should reduce weekly Soll (before={before}, after={after})"
    );
    assert!(after >= 5 * 6 * 60, "five workdays at 6h");
}
