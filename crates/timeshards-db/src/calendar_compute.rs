//! Timesheet evaluation via work calendars (Tagesperiode / Jahresperiode foundation).

use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::SqlitePool;

use crate::absence_eval::{absence_zeros_expected, approved_absence_by_date};
use crate::policy::{evaluate_work_week, load_active_policy, DePolicyRules};
use crate::settlement::load_settlement_config;
use crate::timesheet_compute::compute_daily_worked_minutes;
use crate::work_calendar::{resolve_day, resolve_employee_calendar, scale_expected, week_dates};
use crate::work_model::{
    CalendarDayKind, DayEvaluation, WeekEvaluationMeta, WeekSettlementSummary,
};

pub struct WeekComputation {
    pub worked_minutes: i64,
    pub expected_minutes: i64,
    pub balance_minutes: i64,
    pub overtime_minutes: i64,
    pub warnings: Vec<String>,
    pub meta: Option<WeekEvaluationMeta>,
}

pub async fn compute_week_for_employee(
    pool: &SqlitePool,
    employee_id: &str,
    period_start: DateTime<Utc>,
) -> anyhow::Result<WeekComputation> {
    let period_end = period_start + Duration::days(7);
    let policy = load_active_policy(pool).await?;
    let (daily_work, daily_break) =
        compute_daily_worked_minutes(pool, employee_id, period_start, period_end).await?;

    let dates = week_dates(period_start);
    let on_date = dates[0];
    let Some(ctx) = resolve_employee_calendar(pool, employee_id, on_date).await? else {
        return Ok(fallback_policy_only(&daily_work, &daily_break, &policy));
    };

    let mut days = Vec::with_capacity(7);
    let mut calendar_warnings = Vec::new();
    let absence_by_date = approved_absence_by_date(pool, employee_id, &dates).await?;

    for (i, date) in dates.iter().enumerate() {
        let mut worked = daily_work.get(i).copied().unwrap_or(0);
        let break_mins = daily_break.get(i).copied().unwrap_or(0);

        let Some(resolved) = resolve_day(pool, &ctx, *date).await? else {
            calendar_warnings.push(format!(
                "{}: kein Kalendertag — Soll nicht berechnet",
                date.format("%Y-%m-%d")
            ));
            days.push(DayEvaluation {
                date: date.format("%Y-%m-%d").to_string(),
                weekday: date.weekday().num_days_from_monday() as u8 + 1,
                model_id: String::new(),
                model_name: String::new(),
                day_kind: crate::work_model::CalendarDayKind::Rest,
                expected_minutes: 0,
                worked_minutes: worked,
                break_minutes: break_mins,
                credited_minutes: 0,
                balance_minutes: worked,
                absence_type: None,
                absence_label: None,
                warnings: vec!["Kalendertag fehlt".into()],
            });
            continue;
        };

        if let Some(step) = resolved.config.worked_rounding_minutes.filter(|s| *s > 0) {
            worked = crate::work_model::round_minutes_to_step(worked, step);
        }

        let mut day_expected = scale_expected(&resolved.config, ctx.part_time_percent);
        let absence = absence_by_date.get(date);

        let mut day_kind = resolved.day_kind;
        let mut model_name = resolved.model_name.clone();
        let (absence_type, absence_label) = if let Some(abs) = absence {
            day_kind = CalendarDayKind::Absence;
            model_name = format!("{} ({})", model_name, abs.label);
            (Some(abs.absence_type.clone()), Some(abs.label.clone()))
        } else {
            (None, None)
        };

        if let Some(abs) = absence {
            if absence_zeros_expected(&abs.absence_type) {
                day_expected = 0;
            }
        }

        let mut credited = if resolved.config.auto_credit_expected {
            day_expected
        } else {
            0
        };
        if let Some(abs) = absence {
            if abs.paid_credit && day_expected > 0 {
                credited = day_expected;
            }
        }

        let mut day_warnings = Vec::new();
        if day_expected == 0 && worked > 0 && absence.is_none() {
            day_warnings.push("Arbeit an einem Ruhetag erfasst".into());
        }
        let on_paid_leave = absence.map(|a| a.paid_credit).unwrap_or(false);
        let on_unpaid = absence.map(|a| absence_zeros_expected(&a.absence_type)).unwrap_or(false);
        if day_expected > 0 && worked == 0 && credited == 0 && !on_paid_leave && !on_unpaid {
            day_warnings.push(format!(
                "Soll {} Min — keine Zeiterfassung",
                day_expected
            ));
        }
        if let Some(be) = &resolved.config.break_expectation {
            if worked >= be.required_after_minutes && break_mins < be.required_minutes {
                day_warnings.push(format!(
                    "Pause: {} Min erwartet, {} Min gebucht",
                    be.required_minutes, break_mins
                ));
            }
        }

        let balance = crate::work_model::day_balance_minutes(worked, credited, day_expected);
        days.push(DayEvaluation {
            date: date.format("%Y-%m-%d").to_string(),
            weekday: date.weekday().num_days_from_monday() as u8 + 1,
            model_id: resolved.model_id,
            model_name,
            day_kind,
            expected_minutes: day_expected,
            worked_minutes: worked,
            break_minutes: break_mins,
            credited_minutes: credited,
            balance_minutes: balance,
            absence_type,
            absence_label,
            warnings: day_warnings,
        });
    }

    let worked_total: i64 = days.iter().map(|d| d.worked_minutes).sum();
    let expected_total: i64 = days.iter().map(|d| d.expected_minutes).sum();
    let credited_total: i64 = days.iter().map(|d| d.credited_minutes).sum();
    let balance_total: i64 = days.iter().map(|d| d.balance_minutes).sum();

    let (_, policy_overtime, policy_warnings) =
        evaluate_work_week(&daily_work, &daily_break, &policy);

    let accountable = worked_total + credited_total;
    let calendar_overtime = (accountable - expected_total).max(0);
    let overtime = policy_overtime.max(calendar_overtime);

    let mut warnings = policy_warnings;
    for d in &days {
        for w in &d.warnings {
            warnings.push(format!("{}: {w}", d.date));
        }
    }
    warnings.extend(calendar_warnings);

    if let Some(rule_id) = &ctx.settlement_rule_id {
        let cfg = load_settlement_config(pool, rule_id).await?;
        if cfg.warn_negative_balance && balance_total < 0 {
            warnings.push(format!(
                "Wochensaldo: {} Min Unterdeckung (Soll > Ist+Gutschrift)",
                -balance_total
            ));
        }
    }

    let settlement = WeekSettlementSummary {
        worked_minutes: worked_total,
        expected_minutes: expected_total,
        credited_minutes: credited_total,
        balance_minutes: balance_total,
        overtime_minutes: overtime,
        week_close_weekday: ctx.week_close_weekday,
    };

    let meta = WeekEvaluationMeta {
        work_calendar_id: ctx.work_calendar_id.clone(),
        work_calendar_name: ctx.work_calendar_name,
        part_time_percent: ctx.part_time_percent,
        settlement,
        days,
    };

    Ok(WeekComputation {
        worked_minutes: worked_total,
        expected_minutes: expected_total,
        balance_minutes: balance_total,
        overtime_minutes: overtime,
        warnings,
        meta: Some(meta),
    })
}

fn fallback_policy_only(
    daily_work: &[i64],
    daily_break: &[i64],
    policy: &DePolicyRules,
) -> WeekComputation {
    let (worked, overtime, mut warnings) = evaluate_work_week(daily_work, daily_break, policy);
    warnings.insert(
        0,
        "Kein Arbeitskalender — Sollzeit nicht berechnet (nur ArbZG-Prüfung)".into(),
    );
    WeekComputation {
        worked_minutes: worked,
        expected_minutes: 0,
        balance_minutes: worked,
        overtime_minutes: overtime,
        warnings,
        meta: None,
    }
}

pub async fn upsert_timesheet_from_computation(
    pool: &SqlitePool,
    employee_id: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    comp: &WeekComputation,
) -> anyhow::Result<bool> {
    let ps = period_start.to_rfc3339();
    let pe = period_end.to_rfc3339();
    let evaluation_json = comp
        .meta
        .as_ref()
        .map(|m| serde_json::to_string(m))
        .transpose()?;

    let existing: Option<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM timesheets WHERE employee_id = ? AND period_start = ?",
    )
    .bind(employee_id)
    .bind(&ps)
    .fetch_optional(pool)
    .await?;

    if let Some((_, status)) = &existing {
        if status == "approved" {
            return Ok(false);
        }
    }

    let credited_week: i64 = comp
        .meta
        .as_ref()
        .map(|m| m.days.iter().map(|d| d.credited_minutes).sum())
        .unwrap_or(0);
    if comp.worked_minutes == 0 && comp.expected_minutes == 0 && credited_week == 0 {
        return Ok(false);
    }

    let now = Utc::now().to_rfc3339();

    if let Some((ts_id, _)) = existing {
        sqlx::query(
            r#"
            UPDATE timesheets
            SET period_end = ?, worked_minutes = ?, expected_minutes = ?, balance_minutes = ?,
                overtime_minutes = ?, evaluation_json = ?,
                status = CASE WHEN status IN ('approved', 'pending') THEN status ELSE 'draft' END
            WHERE id = ?
            "#,
        )
        .bind(&pe)
        .bind(comp.worked_minutes)
        .bind(comp.expected_minutes)
        .bind(comp.balance_minutes)
        .bind(comp.overtime_minutes)
        .bind(&evaluation_json)
        .bind(ts_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO timesheets (
                id, employee_id, period_start, period_end,
                worked_minutes, expected_minutes, balance_minutes, overtime_minutes,
                evaluation_json, status, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(employee_id)
        .bind(&ps)
        .bind(&pe)
        .bind(comp.worked_minutes)
        .bind(comp.expected_minutes)
        .bind(comp.balance_minutes)
        .bind(comp.overtime_minutes)
        .bind(&evaluation_json)
        .bind(&now)
        .execute(pool)
        .await?;
    }
    Ok(true)
}
