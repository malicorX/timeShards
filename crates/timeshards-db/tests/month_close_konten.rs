//! Month close must not double-post flex when weekly approvals already match month balance.

use chrono::{Datelike, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{
    close_month, ensure_work_calendar_foundation, post_timesheet_approval, preview_month,
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
async fn month_close_skips_duplicate_flex_posting() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T97', 'Month MA', ?, ?)",
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

    let year = Utc::now().year();
    let month = Utc::now().month();
    let ps = format!("{year}-{month:02}-01T00:00:00Z");
    let pe = if month == 12 {
        format!("{}-01-01T00:00:00Z", year + 1)
    } else {
        format!("{year}-{:02}-01T00:00:00Z", month + 1)
    };
    let ts_id = uuid::Uuid::new_v4().to_string();
    let balance = 120i64;

    sqlx::query(
        r#"
        INSERT INTO timesheets (
            id, employee_id, period_start, period_end,
            worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
            status, created_at
        ) VALUES (?, ?, ?, ?, 600, 480, ?, 0, 'approved', ?)
        "#,
    )
    .bind(&ts_id)
    .bind(&emp_id)
    .bind(&ps)
    .bind(&pe)
    .bind(balance)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    post_timesheet_approval(&pool, &ts_id, &emp_id, &ps, balance, 0)
        .await
        .unwrap();

    let flex_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM time_account_entries WHERE employee_id = ? AND account_kind = 'flex'",
    )
    .bind(&emp_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(flex_before, 1);

    let preview = preview_month(&pool, &emp_id, year, month).await.unwrap();
    assert_eq!(preview.balance_minutes, balance);

    close_month(&pool, &emp_id, year, month, None)
        .await
        .expect("close month");

    let flex_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM time_account_entries WHERE employee_id = ? AND account_kind = 'flex'",
    )
    .bind(&emp_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        flex_after, 1,
        "month close must not add duplicate flex entry when weekly posting matches"
    );
}
