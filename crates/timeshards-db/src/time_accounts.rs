//! Time accounts (flex saldo, overtime) — weekly approve + monthly reconciliation.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

pub const ACCOUNT_FLEX: &str = "flex";
pub const ACCOUNT_OVERTIME: &str = "overtime";

/// Book weekly flex balance and overtime from an approved timesheet (idempotent per timesheet).
pub async fn post_timesheet_approval(
    pool: &SqlitePool,
    timesheet_id: &str,
    employee_id: &str,
    period_start: &str,
    balance_minutes: i64,
    overtime_minutes: i64,
) -> anyhow::Result<()> {
    if balance_minutes != 0 {
        post_timesheet_entry(
            pool,
            timesheet_id,
            employee_id,
            ACCOUNT_FLEX,
            period_start,
            balance_minutes,
            "Wochensaldo aus Stundenzettel",
        )
        .await?;
    }
    if overtime_minutes > 0 {
        post_timesheet_entry(
            pool,
            timesheet_id,
            employee_id,
            ACCOUNT_OVERTIME,
            period_start,
            overtime_minutes,
            "Überstunden aus Stundenzettel",
        )
        .await?;
    }
    Ok(())
}

/// After month close: post only the delta vs sum of weekly flex/ÜS bookings (idempotent per period).
pub async fn post_month_close_reconciliation(
    pool: &SqlitePool,
    settlement_period_id: &str,
    employee_id: &str,
    year: i32,
    month: u32,
    month_balance: i64,
    month_overtime: i64,
) -> anyhow::Result<()> {
    let (from, to) = month_bounds_rfc3339(year, month)?;

    let flex_posted: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(delta_minutes), 0) FROM time_account_entries
        WHERE employee_id = ? AND account_kind = ?
          AND timesheet_id IS NOT NULL
          AND period_start >= ? AND period_start < ?
        "#,
    )
    .bind(employee_id)
    .bind(ACCOUNT_FLEX)
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;

    let flex_adj = month_balance - flex_posted;
    if flex_adj != 0 {
        post_settlement_entry(
            pool,
            settlement_period_id,
            employee_id,
            ACCOUNT_FLEX,
            &from,
            flex_adj,
            &format!("Monatsausgleich Gleitzeit {month:02}/{year}"),
        )
        .await?;
    }

    let ot_posted: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(delta_minutes), 0) FROM time_account_entries
        WHERE employee_id = ? AND account_kind = ?
          AND timesheet_id IS NOT NULL
          AND period_start >= ? AND period_start < ?
        "#,
    )
    .bind(employee_id)
    .bind(ACCOUNT_OVERTIME)
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;

    let ot_adj = month_overtime - ot_posted;
    if ot_adj != 0 {
        post_settlement_entry(
            pool,
            settlement_period_id,
            employee_id,
            ACCOUNT_OVERTIME,
            &from,
            ot_adj,
            &format!("Monatsausgleich Überstunden {month:02}/{year}"),
        )
        .await?;
    }

    Ok(())
}

fn month_bounds_rfc3339(year: i32, month: u32) -> anyhow::Result<(String, String)> {
    let start = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| anyhow::anyhow!("Ungültiges Jahr/Monat"))?;
    let end = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .ok_or_else(|| anyhow::anyhow!("Ungültiges Jahr/Monat"))?;
    Ok((
        start.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339(),
        end.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339(),
    ))
}

async fn post_timesheet_entry(
    pool: &SqlitePool,
    timesheet_id: &str,
    employee_id: &str,
    account_kind: &str,
    period_start: &str,
    delta_minutes: i64,
    note: &str,
) -> anyhow::Result<()> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM time_account_entries
        WHERE timesheet_id = ? AND account_kind = ?
        "#,
    )
    .bind(timesheet_id)
    .bind(account_kind)
    .fetch_one(pool)
    .await?;
    if exists > 0 {
        return Ok(());
    }

    insert_entry(
        pool,
        employee_id,
        account_kind,
        Some(timesheet_id),
        None,
        period_start,
        delta_minutes,
        note,
    )
    .await
}

async fn post_settlement_entry(
    pool: &SqlitePool,
    settlement_period_id: &str,
    employee_id: &str,
    account_kind: &str,
    period_start: &str,
    delta_minutes: i64,
    note: &str,
) -> anyhow::Result<()> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM time_account_entries
        WHERE settlement_period_id = ? AND account_kind = ?
        "#,
    )
    .bind(settlement_period_id)
    .bind(account_kind)
    .fetch_one(pool)
    .await?;
    if exists > 0 {
        return Ok(());
    }

    insert_entry(
        pool,
        employee_id,
        account_kind,
        None,
        Some(settlement_period_id),
        period_start,
        delta_minutes,
        note,
    )
    .await
}

async fn insert_entry(
    pool: &SqlitePool,
    employee_id: &str,
    account_kind: &str,
    timesheet_id: Option<&str>,
    settlement_period_id: Option<&str>,
    period_start: &str,
    delta_minutes: i64,
    note: &str,
) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    let entry_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO time_account_entries (
            id, employee_id, account_kind, timesheet_id, settlement_period_id,
            period_start, delta_minutes, note, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&entry_id)
    .bind(employee_id)
    .bind(account_kind)
    .bind(timesheet_id)
    .bind(settlement_period_id)
    .bind(period_start)
    .bind(delta_minutes)
    .bind(note)
    .bind(&now)
    .execute(pool)
    .await?;

    apply_balance_delta(pool, employee_id, account_kind, delta_minutes, &now).await
}

async fn apply_balance_delta(
    pool: &SqlitePool,
    employee_id: &str,
    account_kind: &str,
    delta_minutes: i64,
    now: &str,
) -> anyhow::Result<()> {
    let updated: i64 = sqlx::query_scalar(
        "SELECT balance_minutes FROM time_accounts WHERE employee_id = ? AND account_kind = ?",
    )
    .bind(employee_id)
    .bind(account_kind)
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    let new_balance = updated + delta_minutes;

    sqlx::query(
        r#"
        INSERT INTO time_accounts (employee_id, account_kind, balance_minutes, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(employee_id, account_kind) DO UPDATE SET
            balance_minutes = excluded.balance_minutes,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(employee_id)
    .bind(account_kind)
    .bind(new_balance)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_account_balances(
    pool: &SqlitePool,
    employee_id: &str,
) -> anyhow::Result<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT account_kind, balance_minutes FROM time_accounts WHERE employee_id = ? ORDER BY account_kind",
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
