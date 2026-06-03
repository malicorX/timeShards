//! Integration: clock_out triggers timesheet rebuild with Ist/Soll on draft row.

use chrono::{Datelike, Duration, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{
    ensure_work_calendar_foundation, rebuild_timesheet_for_employee_week,
    rebuild_timesheets_for_employee_recent, week_bounds_utc, DEFAULT_WORK_CALENDAR_ID,
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
async fn rebuild_after_punch_writes_expected_and_worked() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T97', 'Rebuild MA', ?, ?)",
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
    let day = week_start + Duration::hours(9);
    let day_end = day + Duration::hours(4);

    for (kind, at) in [("clock_in", day), ("clock_out", day_end)] {
        sqlx::query(
            r#"
            INSERT INTO time_events (id, employee_id, kind, occurred_at, source, notes, created_at)
            VALUES (?, ?, ?, ?, 'test', NULL, ?)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&emp_id)
        .bind(kind)
        .bind(at.to_rfc3339())
        .bind(at.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    }

    let (updated, _) = rebuild_timesheet_for_employee_week(&pool, &emp_id, week_start)
        .await
        .unwrap();
    assert_eq!(updated, 1, "draft timesheet should be created/updated");

    let row: (i64, i64, i64, String) = sqlx::query_as(
        r#"
        SELECT worked_minutes, expected_minutes, balance_minutes, status
        FROM timesheets
        WHERE employee_id = ? AND period_start = ?
        "#,
    )
    .bind(&emp_id)
    .bind(&period_start)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 4 * 60, "worked from punch pair");
    assert!(row.1 >= 5 * 8 * 60, "Mo–Fr Soll for week");
    assert_eq!(row.3, "draft");
    assert!(row.2 < row.1, "under Soll → negative balance");
}

#[tokio::test]
async fn employee_recent_rebuild_creates_current_week_without_punches() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T98', 'Recent MA', ?, ?)",
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

    let (updated, _) = rebuild_timesheets_for_employee_recent(&pool, &emp_id, 1)
        .await
        .unwrap();
    assert!(updated >= 1, "at least current week draft with Soll");

    let (week_start, _) = week_bounds_utc(Utc::now());
    let expected: i64 = sqlx::query_scalar(
        "SELECT expected_minutes FROM timesheets WHERE employee_id = ? AND period_start = ?",
    )
    .bind(&emp_id)
    .bind(week_start.to_rfc3339())
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(expected >= 5 * 8 * 60, "Mo–Fr Soll without punches");
}
