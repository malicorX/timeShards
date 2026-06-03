//! Flex-band advisories at punch time (advisory or enforced via settlement rule).

use chrono::{DateTime, Utc};
use chrono_tz::Europe::Berlin;
use sqlx::SqlitePool;

use crate::settlement::load_settlement_config;
use crate::work_calendar::{resolve_day, resolve_employee_calendar};
use crate::work_model::flex_band_advisory;

pub struct PunchFlexCheck {
    pub advisory: Option<String>,
    pub enforce: bool,
}

pub async fn punch_flex_check(
    pool: &SqlitePool,
    employee_id: &str,
    kind: &str,
    occurred_at: DateTime<Utc>,
) -> anyhow::Result<PunchFlexCheck> {
    if kind != "clock_in" && kind != "clock_out" {
        return Ok(PunchFlexCheck {
            advisory: None,
            enforce: false,
        });
    }
    let date = occurred_at.with_timezone(&Berlin).date_naive();
    let Some(ctx) = resolve_employee_calendar(pool, employee_id, date).await? else {
        return Ok(PunchFlexCheck {
            advisory: Some(
                "Kein Arbeitskalender zugewiesen — Sollzeit wird nicht berechnet".into(),
            ),
            enforce: false,
        });
    };
    let enforce = if let Some(rule_id) = &ctx.settlement_rule_id {
        load_settlement_config(pool, rule_id)
            .await
            .map(|c| c.enforce_flex_band)
            .unwrap_or(false)
    } else {
        false
    };
    let Some(resolved) = resolve_day(pool, &ctx, date).await? else {
        return Ok(PunchFlexCheck {
            advisory: None,
            enforce,
        });
    };
    let local_hhmm = occurred_at.with_timezone(&Berlin).format("%H:%M").to_string();
    let advisory = flex_band_advisory(&local_hhmm, &resolved.config);
    Ok(PunchFlexCheck { advisory, enforce })
}

pub async fn punch_flex_advisory(
    pool: &SqlitePool,
    employee_id: &str,
    kind: &str,
    occurred_at: DateTime<Utc>,
) -> anyhow::Result<Option<String>> {
    Ok(
        punch_flex_check(pool, employee_id, kind, occurred_at)
            .await?
            .advisory,
    )
}
