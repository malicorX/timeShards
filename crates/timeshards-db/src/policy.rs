use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct DePolicyRules {
    pub max_daily_minutes: i64,
    pub max_weekly_minutes: i64,
    pub weekly_regular_minutes: i64,
    pub daily_regular_minutes: i64,
    pub min_break_after_minutes: i64,
    pub min_break_minutes: i64,
}

impl Default for DePolicyRules {
    fn default() -> Self {
        Self {
            max_daily_minutes: 600,
            max_weekly_minutes: 2880,
            weekly_regular_minutes: 40 * 60,
            daily_regular_minutes: 8 * 60,
            min_break_after_minutes: 6 * 60,
            min_break_minutes: 30,
        }
    }
}

#[derive(Deserialize)]
struct PolicyJson {
    #[serde(default = "default_max_daily")]
    max_daily_minutes: i64,
    #[serde(default = "default_max_weekly")]
    max_weekly_minutes: i64,
}

fn default_max_daily() -> i64 {
    600
}
fn default_max_weekly() -> i64 {
    2880
}

pub async fn load_active_policy(pool: &SqlitePool) -> anyhow::Result<DePolicyRules> {
    let row: Option<String> = sqlx::query_scalar(
        "SELECT rules_json FROM policy_packs WHERE active = 1 ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    let mut rules = DePolicyRules::default();
    if let Some(json) = row {
        if let Ok(parsed) = serde_json::from_str::<PolicyJson>(&json) {
            rules.max_daily_minutes = parsed.max_daily_minutes;
            rules.max_weekly_minutes = parsed.max_weekly_minutes;
        }
    }
    Ok(rules)
}

/// Returns (total_worked, overtime_minutes, warnings).
pub fn evaluate_work_week(
    daily_minutes: &[i64],
    daily_break_minutes: &[i64],
    rules: &DePolicyRules,
) -> (i64, i64, Vec<String>) {
    let mut warnings = Vec::new();
    let mut total: i64 = 0;
    let mut daily_overtime_sum: i64 = 0;

    for (i, &mins) in daily_minutes.iter().enumerate() {
        total += mins;
        if mins > rules.max_daily_minutes {
            warnings.push(format!(
                "Tag {}: {} Min über ArbZG-Tageshöchstzeit ({} h)",
                i + 1,
                mins - rules.max_daily_minutes,
                rules.max_daily_minutes / 60
            ));
        }
        if mins > rules.daily_regular_minutes {
            daily_overtime_sum += mins - rules.daily_regular_minutes;
        }
        let break_mins = daily_break_minutes.get(i).copied().unwrap_or(0);
        if mins >= rules.min_break_after_minutes && break_mins < rules.min_break_minutes {
            warnings.push(format!(
                "Tag {}: Arbeitszeit ≥ {} h — Pause ({} Min) nicht erfasst (gebucht: {} Min)",
                i + 1,
                rules.min_break_after_minutes / 60,
                rules.min_break_minutes,
                break_mins
            ));
        }
    }

    if total > rules.max_weekly_minutes {
        warnings.push(format!(
            "Woche: {} Min über Wochenhöchstzeit ({} h)",
            total - rules.max_weekly_minutes,
            rules.max_weekly_minutes / 60
        ));
    }

    let weekly_overtime = (total - rules.weekly_regular_minutes).max(0);
    let overtime = daily_overtime_sum.max(weekly_overtime);

    (total, overtime, warnings)
}
