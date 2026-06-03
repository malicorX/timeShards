//! Work rotation plans (Umschaltplan): cyclic Tagesmodell slots from an anchor date.

use chrono::NaiveDate;
use sqlx::SqlitePool;

use crate::work_model::{parse_workday_config, CalendarDayKind, WorkdayModelConfig};

pub const DEFAULT_ROTATION_PLAN_ID: &str = "rp-14day-alt";

pub fn rotation_slot_index(anchor: NaiveDate, date: NaiveDate, cycle_days: i32) -> i32 {
    if cycle_days <= 0 {
        return 0;
    }
    let days = (date - anchor).num_days();
    days.rem_euclid(cycle_days as i64) as i32
}

pub async fn resolve_rotation_day(
    pool: &SqlitePool,
    plan_id: &str,
    anchor_date: NaiveDate,
    cycle_days: i32,
    date: NaiveDate,
) -> anyhow::Result<Option<(String, String, WorkdayModelConfig)>> {
    let slot = rotation_slot_index(anchor_date, date, cycle_days);
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT s.workday_model_id, m.name
        FROM work_rotation_slots s
        JOIN workday_models m ON m.id = s.workday_model_id
        WHERE s.plan_id = ? AND s.slot_index = ?
        "#,
    )
    .bind(plan_id)
    .bind(slot)
    .fetch_optional(pool)
    .await?;

    let Some((model_id, model_name)) = row else {
        return Ok(None);
    };
    let json: String = sqlx::query_scalar("SELECT config_json FROM workday_models WHERE id = ?")
        .bind(&model_id)
        .fetch_one(pool)
        .await?;
    let config = parse_workday_config(&json)?;
    Ok(Some((model_id, model_name, config)))
}

pub fn day_kind_for_model(config: &WorkdayModelConfig) -> CalendarDayKind {
    if config.expected_minutes > 0 {
        CalendarDayKind::Work
    } else {
        CalendarDayKind::Rest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_index_wraps_cycle() {
        let anchor = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(); // Monday
        let d = NaiveDate::from_ymd_opt(2026, 1, 19).unwrap(); // +14 days
        assert_eq!(rotation_slot_index(anchor, d, 14), 0);
        assert_eq!(rotation_slot_index(anchor, anchor, 14), 0);
        assert_eq!(rotation_slot_index(anchor, anchor + chrono::Duration::days(1), 14), 1);
    }
}
