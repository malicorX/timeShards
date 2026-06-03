//! Default work-calendar assignment for employees without an active assignment.

use chrono::{Datelike, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{
    assign_all_active_without_work_calendar, ensure_work_calendar_foundation,
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
async fn assign_all_active_without_calendar_is_idempotent() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T96', 'No Cal MA', ?, ?)",
    )
    .bind(&emp_id)
    .bind(&valid_from)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let first = assign_all_active_without_work_calendar(&pool).await.unwrap();
    assert_eq!(first, 1, "new employee should get default calendar");

    let has: String = sqlx::query_scalar(
        "SELECT work_calendar_id FROM employee_work_assignments WHERE employee_id = ? LIMIT 1",
    )
    .bind(&emp_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(has, DEFAULT_WORK_CALENDAR_ID);

    let second = assign_all_active_without_work_calendar(&pool).await.unwrap();
    assert_eq!(second, 0, "second run must not duplicate assignments");
}
