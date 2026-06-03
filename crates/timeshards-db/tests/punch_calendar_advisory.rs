//! Punch advisory when employee has no work-calendar assignment.

use chrono::Utc;
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{ensure_work_calendar_foundation, punch_flex_check};

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("memory db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrate");
    pool
}

#[tokio::test]
async fn punch_without_calendar_returns_advisory() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T95', 'No Cal', ?, ?)",
    )
    .bind(&emp_id)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let check = punch_flex_check(&pool, &emp_id, "clock_in", Utc::now())
        .await
        .unwrap();
    let msg = check.advisory.expect("expected advisory");
    assert!(
        msg.contains("Arbeitskalender"),
        "unexpected advisory: {msg}"
    );
    assert!(!check.enforce);
}
