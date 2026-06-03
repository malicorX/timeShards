//! Resolve work calendars and day models for an employee.

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Europe::Berlin;
use sqlx::SqlitePool;

use crate::work_model::{parse_workday_config, CalendarDayKind, WorkdayModelConfig};
use crate::work_rotation::{day_kind_for_model, resolve_rotation_day};

pub struct EmployeeCalendarContext {
    pub assignment_id: String,
    pub work_calendar_id: String,
    pub work_calendar_name: String,
    pub holiday_calendar_id: Option<String>,
    pub part_time_percent: i32,
    pub week_close_weekday: i32,
    pub settlement_rule_id: Option<String>,
    pub rotation_plan_id: Option<String>,
    pub rotation_anchor_date: Option<NaiveDate>,
    pub rotation_cycle_days: Option<i32>,
}

pub struct ResolvedDay {
    pub date: NaiveDate,
    pub model_id: String,
    pub model_name: String,
    pub config: WorkdayModelConfig,
    pub day_kind: CalendarDayKind,
}

pub async fn resolve_employee_calendar(
    pool: &SqlitePool,
    employee_id: &str,
    on_date: NaiveDate,
) -> anyhow::Result<Option<EmployeeCalendarContext>> {
    let date_str = on_date.format("%Y-%m-%d").to_string();
    let row: Option<(
        String,
        String,
        String,
        Option<String>,
        i32,
        i32,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
            r#"
        SELECT a.id, a.work_calendar_id, c.name, c.holiday_calendar_id, a.part_time_percent,
               c.week_close_weekday, c.settlement_rule_id, c.rotation_plan_id
        FROM employee_work_assignments a
        JOIN work_calendars c ON c.id = a.work_calendar_id
        WHERE a.employee_id = ?
          AND a.valid_from <= ?
          AND (a.valid_to IS NULL OR a.valid_to > ?)
        ORDER BY a.valid_from DESC
        LIMIT 1
        "#,
        )
        .bind(employee_id)
        .bind(&date_str)
        .bind(&date_str)
        .fetch_optional(pool)
        .await?;

    let mut ctx = row.map(
        |(
            assignment_id,
            work_calendar_id,
            work_calendar_name,
            holiday_calendar_id,
            part_time_percent,
            week_close_weekday,
            settlement_rule_id,
            rotation_plan_id,
        )| EmployeeCalendarContext {
            assignment_id,
            work_calendar_id,
            work_calendar_name,
            holiday_calendar_id,
            part_time_percent,
            week_close_weekday,
            settlement_rule_id,
            rotation_plan_id: rotation_plan_id.clone(),
            rotation_anchor_date: None,
            rotation_cycle_days: None,
        },
    );

    if let Some(ctx) = ctx.as_mut() {
        if let Some(plan_id) = &ctx.rotation_plan_id {
            let plan_row: Option<(String, i32)> = sqlx::query_as(
                "SELECT anchor_date, cycle_days FROM work_rotation_plans WHERE id = ?",
            )
            .bind(plan_id)
            .fetch_optional(pool)
            .await?;
            if let Some((anchor, cycle)) = plan_row {
                ctx.rotation_anchor_date =
                    NaiveDate::parse_from_str(&anchor, "%Y-%m-%d").ok();
                ctx.rotation_cycle_days = Some(cycle);
            }
        }
    }

    Ok(ctx)
}

pub async fn resolve_day(
    pool: &SqlitePool,
    ctx: &EmployeeCalendarContext,
    date: NaiveDate,
) -> anyhow::Result<Option<ResolvedDay>> {
    let date_str = date.format("%Y-%m-%d").to_string();

    if let Some(hc_id) = &ctx.holiday_calendar_id {
        let holiday: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT h.day_kind, h.name, h.workday_model_id
            FROM holiday_calendar_days h
            WHERE h.calendar_id = ? AND h.date = ?
            "#,
        )
        .bind(hc_id)
        .bind(&date_str)
        .fetch_optional(pool)
        .await?;

        if let Some((day_kind, _name, model_override)) = holiday {
            let (model_id, model_name, config, kind) =
                holiday_day_model(pool, &day_kind, model_override.as_deref()).await?;
            return Ok(Some(ResolvedDay {
                date,
                model_id,
                model_name,
                config,
                day_kind: kind,
            }));
        }
    }

    if let (Some(plan_id), Some(anchor), Some(cycle)) = (
        &ctx.rotation_plan_id,
        ctx.rotation_anchor_date,
        ctx.rotation_cycle_days,
    ) {
        if let Some((model_id, model_name, config)) =
            resolve_rotation_day(pool, plan_id, anchor, cycle, date).await?
        {
            return Ok(Some(ResolvedDay {
                date,
                model_id,
                model_name,
                config: config.clone(),
                day_kind: day_kind_for_model(&config),
            }));
        }
    }

    let cal_day: Option<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT d.workday_model_id, m.name, m.config_json
        FROM work_calendar_days d
        JOIN workday_models m ON m.id = d.workday_model_id
        WHERE d.calendar_id = ? AND d.date = ?
        "#,
    )
    .bind(&ctx.work_calendar_id)
    .bind(&date_str)
    .fetch_optional(pool)
    .await?;

    let Some((model_id, model_name, config_json)) = cal_day else {
        return Ok(None);
    };
    let config = parse_workday_config(&config_json)?;
    let day_kind = day_kind_for_model(&config);
    Ok(Some(ResolvedDay {
        date,
        model_id,
        model_name,
        config,
        day_kind,
    }))
}

async fn holiday_day_model(
    pool: &SqlitePool,
    day_kind: &str,
    model_override: Option<&str>,
) -> anyhow::Result<(String, String, WorkdayModelConfig, CalendarDayKind)> {
    if let Some(mid) = model_override {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT name, config_json FROM workday_models WHERE id = ?",
        )
        .bind(mid)
        .fetch_optional(pool)
        .await?;
        if let Some((name, json)) = row {
            return Ok((
                mid.to_string(),
                name,
                parse_workday_config(&json)?,
                CalendarDayKind::Holiday,
            ));
        }
    }

    let default_id = match day_kind {
        "special_work" => "wm-std-8h",
        _ => "wm-holiday-paid",
    };
    let row: (String, String) = sqlx::query_as(
        "SELECT name, config_json FROM workday_models WHERE id = ?",
    )
    .bind(default_id)
    .fetch_one(pool)
    .await?;
    let kind = if day_kind == "special_work" {
        CalendarDayKind::Work
    } else {
        CalendarDayKind::Holiday
    };
    Ok((
        default_id.into(),
        row.0,
        parse_workday_config(&row.1)?,
        kind,
    ))
}

pub fn scale_expected(config: &WorkdayModelConfig, part_time_percent: i32) -> i64 {
    let pct = part_time_percent.clamp(1, 100) as i64;
    config.expected_minutes * pct / 100
}

/// Monday (calendar date) of the week containing `period_start`, in Europe/Berlin.
pub fn berlin_week_monday(period_start: DateTime<Utc>) -> NaiveDate {
    period_start.with_timezone(&Berlin).date_naive()
}

pub fn week_dates(period_start: DateTime<Utc>) -> Vec<NaiveDate> {
    let monday = berlin_week_monday(period_start);
    (0..7)
        .map(|i| monday + chrono::Duration::days(i))
        .collect()
}

/// Copy workday model assignments from a source date range to a target range (day-aligned).
pub async fn copy_calendar_days(
    pool: &SqlitePool,
    calendar_id: &str,
    source_from: NaiveDate,
    source_to: NaiveDate,
    target_from: NaiveDate,
) -> anyhow::Result<u32> {
    let span = (source_to - source_from).num_days();
    if span < 0 || span > 366 {
        anyhow::bail!("Ungültiger Quellzeitraum");
    }
    let days = span + 1;
    let mut copied = 0u32;
    for offset in 0..days {
        let src = source_from + chrono::Duration::days(offset);
        let tgt = target_from + chrono::Duration::days(offset);
        let src_str = src.format("%Y-%m-%d").to_string();
        let model_id: Option<String> = sqlx::query_scalar(
            "SELECT workday_model_id FROM work_calendar_days WHERE calendar_id = ? AND date = ?",
        )
        .bind(calendar_id)
        .bind(&src_str)
        .fetch_optional(pool)
        .await?;
        let Some(model_id) = model_id else {
            continue;
        };
        let tgt_str = tgt.format("%Y-%m-%d").to_string();
        sqlx::query(
            r#"
            INSERT INTO work_calendar_days (calendar_id, date, workday_model_id)
            VALUES (?, ?, ?)
            ON CONFLICT(calendar_id, date) DO UPDATE SET workday_model_id = excluded.workday_model_id
            "#,
        )
        .bind(calendar_id)
        .bind(&tgt_str)
        .bind(&model_id)
        .execute(pool)
        .await?;
        copied += 1;
    }
    Ok(copied)
}
