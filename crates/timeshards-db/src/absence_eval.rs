//! Approved absences affecting daily Soll / Gutschrift in calendar evaluation.

use std::collections::HashMap;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::SqlitePool;

const TZ: Tz = chrono_tz::Europe::Berlin;

/// Effect of an approved absence on a single calendar day.
#[derive(Debug, Clone)]
pub struct AbsenceDayEffect {
    pub absence_type: String,
    pub label: String,
    /// Full-day paid credit (reduces missing-punch warnings, adds Soll-Gutschrift).
    pub paid_credit: bool,
}

pub fn absence_label(absence_type: &str) -> &'static str {
    match absence_type {
        "urlaub" => "Urlaub",
        "krank" => "Krankheit",
        "sonder" => "Sonderurlaub",
        "unbezahlt" => "Unbezahlt",
        _ => "Abwesenheit",
    }
}

pub fn absence_paid_credit(absence_type: &str) -> bool {
    matches!(absence_type, "urlaub" | "krank" | "sonder")
}

/// Unpaid absence: no Soll on that day (expected zeroed).
pub fn absence_zeros_expected(absence_type: &str) -> bool {
    absence_type == "unbezahlt"
}

fn day_bounds_utc(date: NaiveDate) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let start_naive = date.and_hms_opt(0, 0, 0)?;
    let end_date = date + Duration::days(1);
    let end_naive = end_date.and_hms_opt(0, 0, 0)?;
    let start = start_naive.and_local_timezone(TZ).single()?.with_timezone(&Utc);
    let end = end_naive.and_local_timezone(TZ).single()?.with_timezone(&Utc);
    Some((start, end))
}

fn intervals_overlap(a0: DateTime<Utc>, a1: DateTime<Utc>, b0: DateTime<Utc>, b1: DateTime<Utc>) -> bool {
    a0 < b1 && a1 > b0
}

/// Map each date (YYYY-MM-DD) in the week to an approved absence effect, if any.
pub async fn approved_absence_by_date(
    pool: &SqlitePool,
    employee_id: &str,
    week_dates: &[NaiveDate],
) -> anyhow::Result<HashMap<NaiveDate, AbsenceDayEffect>> {
    if week_dates.is_empty() {
        return Ok(HashMap::new());
    }

    let Some((range_start, _)) = day_bounds_utc(week_dates[0]) else {
        return Ok(HashMap::new());
    };
    let Some((_, range_end)) = day_bounds_utc(*week_dates.last().unwrap()) else {
        return Ok(HashMap::new());
    };

    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT absence_type, starts_at, ends_at
        FROM absence_requests
        WHERE employee_id = ? AND status = 'approved'
          AND starts_at < ? AND ends_at > ?
        "#,
    )
    .bind(employee_id)
    .bind(range_end.to_rfc3339())
    .bind(range_start.to_rfc3339())
    .fetch_all(pool)
    .await?;

    let mut out = HashMap::new();
    for date in week_dates {
        let Some((day_start, day_end)) = day_bounds_utc(*date) else {
            continue;
        };
        for (absence_type, starts_at, ends_at) in &rows {
            let Ok(abs_start) = DateTime::parse_from_rfc3339(starts_at) else {
                continue;
            };
            let Ok(abs_end) = DateTime::parse_from_rfc3339(ends_at) else {
                continue;
            };
            let abs_start = abs_start.with_timezone(&Utc);
            let abs_end = abs_end.with_timezone(&Utc);
            if !intervals_overlap(abs_start, abs_end, day_start, day_end) {
                continue;
            }
            let label = absence_label(absence_type).to_string();
            let paid_credit = absence_paid_credit(absence_type);
            out.insert(
                *date,
                AbsenceDayEffect {
                    absence_type: absence_type.clone(),
                    label,
                    paid_credit,
                },
            );
            break;
        }
    }
    Ok(out)
}
