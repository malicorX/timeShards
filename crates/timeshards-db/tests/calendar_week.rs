//! Integration: work calendar → weekly Soll from Mo–Fr 8h model.

use chrono::{Datelike, Duration, Utc};
use chrono_tz::Europe::Berlin;
use sqlx::sqlite::SqlitePoolOptions;
use timeshards_db::{
    compute_week_for_employee, ensure_work_calendar_foundation, week_bounds_utc,
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
async fn week_expected_minutes_mon_fri_8h() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool)
        .await
        .expect("foundation seed");

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        r#"
        INSERT INTO employees (id, employee_no, display_name, active_from, created_at)
        VALUES (?, 'T99', 'Test MA', ?, ?)
        "#,
    )
    .bind(&emp_id)
    .bind(&valid_from)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("employee");
    sqlx::query(
        r#"
        INSERT INTO employee_work_assignments (
            id, employee_id, work_calendar_id, valid_from, valid_to, part_time_percent, notes, created_at
        ) VALUES (?, ?, ?, ?, NULL, 100, 'test', ?)
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&emp_id)
    .bind(DEFAULT_WORK_CALENDAR_ID)
    .bind(&valid_from)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("assignment");

    let (week_start, _) = week_bounds_utc(Utc::now());
    let comp = compute_week_for_employee(&pool, &emp_id, week_start)
        .await
        .expect("compute");

    assert!(
        comp.meta.is_some(),
        "expected calendar-backed evaluation"
    );
    assert_eq!(
        comp.expected_minutes, 5 * 8 * 60,
        "Mo–Fr 8h Soll expected"
    );
}

#[tokio::test]
async fn week_balance_from_punches() {
    let pool = memory_pool().await;
    ensure_work_calendar_foundation(&pool).await.unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = format!("{}-01-01", Utc::now().year());
    sqlx::query(
        "INSERT INTO employees (id, employee_no, display_name, active_from, created_at) VALUES (?, 'T98', 'Punch MA', ?, ?)",
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
    let monday_berlin = week_start.with_timezone(&Berlin).date_naive();
    let day = monday_berlin
        .and_hms_opt(8, 0, 0)
        .unwrap()
        .and_local_timezone(Berlin)
        .single()
        .unwrap()
        .with_timezone(&Utc);
    let day_end = day + Duration::hours(8);

    for (kind, at) in [
        ("clock_in", day),
        ("clock_out", day_end),
    ] {
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

    let comp = compute_week_for_employee(&pool, &emp_id, week_start)
        .await
        .unwrap();
    assert_eq!(comp.worked_minutes, 8 * 60);
    let meta = comp.meta.expect("meta");
    let mon = meta.days.first().expect("monday");
    assert_eq!(mon.worked_minutes, 8 * 60);
    assert_eq!(mon.balance_minutes, 0);
}
