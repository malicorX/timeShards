//! Monthly settlement (Monatsperiode): aggregate approved weeks and close.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::time_accounts::post_month_close_reconciliation;
use crate::work_model::WeekEvaluationMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthWeekLine {
    pub timesheet_id: String,
    pub period_start: String,
    pub status: String,
    pub worked_minutes: i64,
    pub expected_minutes: i64,
    pub balance_minutes: i64,
    pub credited_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthSettlementPreview {
    pub year: i32,
    pub month: u32,
    pub employee_id: String,
    pub worked_minutes: i64,
    pub expected_minutes: i64,
    pub balance_minutes: i64,
    pub credited_minutes: i64,
    pub overtime_minutes: i64,
    pub approved_weeks: u32,
    pub pending_weeks: u32,
    pub draft_weeks: u32,
    pub weeks: Vec<MonthWeekLine>,
    pub already_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementPeriodRow {
    pub id: String,
    pub employee_id: String,
    pub year: i32,
    pub month: u32,
    pub status: String,
    pub worked_minutes: i64,
    pub expected_minutes: i64,
    pub balance_minutes: i64,
    pub overtime_minutes: i64,
    pub weeks_count: i32,
    pub closed_at: Option<String>,
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

fn credited_from_evaluation(json: Option<&str>) -> i64 {
    let Some(j) = json else {
        return 0;
    };
    serde_json::from_str::<WeekEvaluationMeta>(j)
        .ok()
        .map(|m| m.settlement.credited_minutes)
        .unwrap_or(0)
}

pub async fn preview_month(
    pool: &SqlitePool,
    employee_id: &str,
    year: i32,
    month: u32,
) -> anyhow::Result<MonthSettlementPreview> {
    if !(1..=12).contains(&month) {
        anyhow::bail!("Monat muss 1–12 sein");
    }
    let (from, to) = month_bounds_rfc3339(year, month)?;

    let closed: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM settlement_periods
        WHERE employee_id = ? AND period_kind = 'month' AND year = ? AND month = ? AND status = 'closed'
        "#,
    )
    .bind(employee_id)
    .bind(year)
    .bind(month as i32)
    .fetch_one(pool)
    .await?;

    let rows: Vec<(
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT id, period_start, status, worked_minutes, expected_minutes, balance_minutes,
               overtime_minutes, evaluation_json
        FROM timesheets
        WHERE employee_id = ? AND period_start >= ? AND period_start < ?
        ORDER BY period_start
        "#,
    )
    .bind(employee_id)
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await?;

    let mut approved_weeks = 0u32;
    let mut pending_weeks = 0u32;
    let mut draft_weeks = 0u32;
    let mut worked_total = 0i64;
    let mut expected_total = 0i64;
    let mut balance_total = 0i64;
    let mut credited_total = 0i64;
    let mut overtime_total = 0i64;
    let mut weeks = Vec::new();

    for (id, period_start, status, worked, expected, balance, overtime, eval_json) in rows {
        let credited = credited_from_evaluation(eval_json.as_deref());
        match status.as_str() {
            "approved" => {
                approved_weeks += 1;
                worked_total += worked;
                expected_total += expected;
                balance_total += balance;
                credited_total += credited;
                overtime_total += overtime;
            }
            "pending" => pending_weeks += 1,
            "draft" => draft_weeks += 1,
            _ => {}
        }
        weeks.push(MonthWeekLine {
            timesheet_id: id,
            period_start,
            status,
            worked_minutes: worked,
            expected_minutes: expected,
            balance_minutes: balance,
            credited_minutes: credited,
        });
    }

    Ok(MonthSettlementPreview {
        year,
        month,
        employee_id: employee_id.to_string(),
        worked_minutes: worked_total,
        expected_minutes: expected_total,
        balance_minutes: balance_total,
        credited_minutes: credited_total,
        overtime_minutes: overtime_total,
        approved_weeks,
        pending_weeks,
        draft_weeks,
        weeks,
        already_closed: closed > 0,
    })
}

pub async fn close_month(
    pool: &SqlitePool,
    employee_id: &str,
    year: i32,
    month: u32,
    closed_by_user_id: Option<&str>,
) -> anyhow::Result<SettlementPeriodRow> {
    let preview = preview_month(pool, employee_id, year, month).await?;
    if preview.already_closed {
        anyhow::bail!("Monat bereits abgeschlossen");
    }
    if preview.pending_weeks > 0 || preview.draft_weeks > 0 {
        anyhow::bail!(
            "Monat nicht abschließbar: {} eingereicht, {} Entwürfe offen",
            preview.pending_weeks,
            preview.draft_weeks
        );
    }
    if preview.approved_weeks == 0 {
        anyhow::bail!("Keine freigegebenen Wochen im Monat");
    }

    let now = Utc::now().to_rfc3339();
    let summary_json = serde_json::to_string(&preview)?;
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO settlement_periods (
            id, employee_id, period_kind, year, month, status,
            worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
            weeks_count, summary_json, closed_at, closed_by_user_id, created_at
        ) VALUES (?, ?, 'month', ?, ?, 'closed', ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(employee_id)
    .bind(year)
    .bind(month as i32)
    .bind(preview.worked_minutes)
    .bind(preview.expected_minutes)
    .bind(preview.balance_minutes)
    .bind(preview.overtime_minutes)
    .bind(preview.approved_weeks as i32)
    .bind(&summary_json)
    .bind(&now)
    .bind(closed_by_user_id)
    .bind(&now)
    .execute(pool)
    .await?;

    post_month_close_reconciliation(
        pool,
        &id,
        employee_id,
        year,
        month,
        preview.balance_minutes,
        preview.overtime_minutes,
    )
    .await?;

    Ok(SettlementPeriodRow {
        id,
        employee_id: employee_id.to_string(),
        year,
        month,
        status: "closed".into(),
        worked_minutes: preview.worked_minutes,
        expected_minutes: preview.expected_minutes,
        balance_minutes: preview.balance_minutes,
        overtime_minutes: preview.overtime_minutes,
        weeks_count: preview.approved_weeks as i32,
        closed_at: Some(now),
    })
}

pub async fn list_closed_periods(
    pool: &SqlitePool,
    year: Option<i32>,
    month: Option<u32>,
    employee_id: Option<&str>,
) -> anyhow::Result<Vec<SettlementPeriodRow>> {
    let rows = match (year, month, employee_id) {
        (Some(y), Some(m), Some(eid)) => {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, year, month, status,
                       worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                       weeks_count, closed_at
                FROM settlement_periods
                WHERE period_kind = 'month' AND status = 'closed'
                  AND year = ? AND month = ? AND employee_id = ?
                ORDER BY year DESC, month DESC
                "#,
            )
            .bind(y)
            .bind(m as i32)
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        (Some(y), Some(m), None) => {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, year, month, status,
                       worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                       weeks_count, closed_at
                FROM settlement_periods
                WHERE period_kind = 'month' AND status = 'closed'
                  AND year = ? AND month = ?
                ORDER BY employee_id
                "#,
            )
            .bind(y)
            .bind(m as i32)
            .fetch_all(pool)
            .await?
        }
        (Some(y), None, Some(eid)) => {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, year, month, status,
                       worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                       weeks_count, closed_at
                FROM settlement_periods
                WHERE period_kind = 'month' AND status = 'closed'
                  AND year = ? AND employee_id = ?
                ORDER BY month DESC
                "#,
            )
            .bind(y)
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        (Some(y), None, None) => {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, year, month, status,
                       worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                       weeks_count, closed_at
                FROM settlement_periods
                WHERE period_kind = 'month' AND status = 'closed' AND year = ?
                ORDER BY month DESC, employee_id
                "#,
            )
            .bind(y)
            .fetch_all(pool)
            .await?
        }
        (None, _, Some(eid)) => {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, year, month, status,
                       worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                       weeks_count, closed_at
                FROM settlement_periods
                WHERE period_kind = 'month' AND status = 'closed' AND employee_id = ?
                ORDER BY year DESC, month DESC
                "#,
            )
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        _ => {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, year, month, status,
                       worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                       weeks_count, closed_at
                FROM settlement_periods
                WHERE period_kind = 'month' AND status = 'closed'
                ORDER BY year DESC, month DESC, employee_id
                LIMIT 200
                "#,
            )
            .fetch_all(pool)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|row| {
            let (
                id,
                employee_id,
                year,
                month,
                status,
                worked_minutes,
                expected_minutes,
                balance_minutes,
                overtime_minutes,
                weeks_count,
                closed_at,
            ): (
                String,
                String,
                i32,
                i32,
                String,
                i64,
                i64,
                i64,
                i64,
                i32,
                Option<String>,
            ) = row;
            SettlementPeriodRow {
                id,
                employee_id,
                year,
                month: month as u32,
                status,
                worked_minutes,
                expected_minutes,
                balance_minutes,
                overtime_minutes,
                weeks_count,
                closed_at,
            }
        })
        .collect())
}
