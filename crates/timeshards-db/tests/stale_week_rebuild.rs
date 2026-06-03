//! Startup-style rebuild for current-week drafts with Soll=0 despite calendar.

use chrono::{Datelike, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{
    ensure_current_week_draft_timesheets, ensure_work_calendar_foundation, week_bounds_utc,
    DEFAULT_WORK_CALENDAR_ID,
};

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn stale_current_week_draft_gets_soll() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T97', 'Stale MA', ?, ?)",
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

    let (week_start, week_end) = week_bounds_utc(Utc::now());
    sqlx::query(
        r#"
        INSERT INTO timesheets (
            id, employee_id, period_start, period_end, worked_minutes, expected_minutes, balance_minutes,
            overtime_minutes, status, created_at
        ) VALUES (?, ?, ?, ?, 0, 0, 0, 0, 'draft', ?)
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&emp_id)
    .bind(week_start.to_rfc3339())
    .bind(week_end.to_rfc3339())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let n = ensure_current_week_draft_timesheets(&pool).await.unwrap();
    assert_eq!(n, 1);

    let expected: i64 = sqlx::query_scalar(
        "SELECT expected_minutes FROM timesheets WHERE employee_id = ? AND period_start = ?",
    )
    .bind(&emp_id)
    .bind(week_start.to_rfc3339())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expected, 5 * 8 * 60);
}

#[tokio::test]
async fn missing_current_week_draft_is_created() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T96', 'No TS MA', ?, ?)",
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
    let n = ensure_current_week_draft_timesheets(&pool).await.unwrap();
    assert_eq!(n, 1);

    let expected: i64 = sqlx::query_scalar(
        "SELECT expected_minutes FROM timesheets WHERE employee_id = ? AND period_start = ?",
    )
    .bind(&emp_id)
    .bind(week_start.to_rfc3339())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expected, 5 * 8 * 60);
}
