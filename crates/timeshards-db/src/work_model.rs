//! Workday model configuration (Tagesperiode-inspired, JSON-backed for flexibility).

use serde::{Deserialize, Serialize};

/// Kind of calendar day after holiday/special resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarDayKind {
    Work,
    Rest,
    Holiday,
    CompanyHoliday,
    Absence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexBand {
    /// HH:MM earliest allowed work start (informational / future enforcement).
    pub earliest_start: String,
    pub latest_end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreTime {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakExpectation {
    pub required_after_minutes: i64,
    pub required_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkdayModelConfig {
    /// Target minutes for the day (Sollzeit). Zero for pure rest days.
    #[serde(default)]
    pub expected_minutes: i64,
    #[serde(default)]
    pub flex_band: Option<FlexBand>,
    #[serde(default)]
    pub core_time: Option<CoreTime>,
    #[serde(default)]
    pub break_expectation: Option<BreakExpectation>,
    /// If true, day counts toward worked time even without punches (holiday credit).
    #[serde(default)]
    pub auto_credit_expected: bool,
    /// Label for UI (e.g. "Büro 8h", "Samstag frei").
    #[serde(default)]
    pub label: Option<String>,
    /// Round daily worked minutes to nearest step (e.g. 5 or 15). None = no rounding.
    #[serde(default)]
    pub worked_rounding_minutes: Option<i32>,
}

impl Default for WorkdayModelConfig {
    fn default() -> Self {
        Self {
            expected_minutes: 8 * 60,
            flex_band: Some(FlexBand {
                earliest_start: "06:00".into(),
                latest_end: "20:00".into(),
            }),
            core_time: Some(CoreTime {
                start: "09:00".into(),
                end: "15:00".into(),
            }),
            break_expectation: Some(BreakExpectation {
                required_after_minutes: 6 * 60,
                required_minutes: 30,
            }),
            auto_credit_expected: false,
            label: None,
            worked_rounding_minutes: None,
        }
    }
}

/// Round minutes to the nearest multiple of `step` (minimum step 1).
/// Returns a German advisory when punch local time is outside the model flex band.
pub fn flex_band_advisory(local_hhmm: &str, config: &WorkdayModelConfig) -> Option<String> {
    let band = config.flex_band.as_ref()?;
    if config.expected_minutes == 0 {
        return None;
    }
    let punch = parse_hhmm(local_hhmm)?;
    let earliest = parse_hhmm(&band.earliest_start)?;
    let latest = parse_hhmm(&band.latest_end)?;
    if punch < earliest {
        return Some(format!(
            "Stempelzeit {local_hhmm} vor Gleitzeitbeginn ({})",
            band.earliest_start
        ));
    }
    if punch > latest {
        return Some(format!(
            "Stempelzeit {local_hhmm} nach Gleitzeitende ({})",
            band.latest_end
        ));
    }
    None
}

fn parse_hhmm(s: &str) -> Option<i32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h: i32 = parts[0].parse().ok()?;
    let m: i32 = parts[1].parse().ok()?;
    Some(h * 60 + m)
}

pub fn round_minutes_to_step(minutes: i64, step: i32) -> i64 {
    let step = step.max(1) as i64;
    ((minutes + step / 2) / step) * step
}

impl WorkdayModelConfig {
    pub fn rest_day() -> Self {
        Self {
            expected_minutes: 0,
            flex_band: None,
            core_time: None,
            break_expectation: None,
            auto_credit_expected: false,
            label: Some("Ruhetag".into()),
            worked_rounding_minutes: None,
        }
    }

    pub fn holiday_paid() -> Self {
        Self {
            expected_minutes: 8 * 60,
            auto_credit_expected: true,
            label: Some("Feiertag (bezahlt)".into()),
            ..Self::default()
        }
    }

    pub fn short_day_6h() -> Self {
        Self {
            expected_minutes: 6 * 60,
            label: Some("Kurztag 6h".into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayEvaluation {
    pub date: String,
    pub weekday: u8,
    pub model_id: String,
    pub model_name: String,
    pub day_kind: CalendarDayKind,
    pub expected_minutes: i64,
    pub worked_minutes: i64,
    pub break_minutes: i64,
    pub credited_minutes: i64,
    pub balance_minutes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absence_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absence_label: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Weekly rollup (Wochenperiode / settlement preview).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekSettlementSummary {
    pub worked_minutes: i64,
    pub expected_minutes: i64,
    pub credited_minutes: i64,
    pub balance_minutes: i64,
    pub overtime_minutes: i64,
    /// chrono weekday 0=Mon … 6=Sun — configured week close on calendar
    pub week_close_weekday: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekEvaluationMeta {
    pub work_calendar_id: String,
    pub work_calendar_name: String,
    pub part_time_percent: i32,
    pub settlement: WeekSettlementSummary,
    pub days: Vec<DayEvaluation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementRuleConfig {
    /// When true, negative weekly balance generates a settlement warning.
    #[serde(default = "default_true")]
    pub warn_negative_balance: bool,
    /// When true, clock-in/out outside flex_band is rejected (otherwise advisory only).
    #[serde(default)]
    pub enforce_flex_band: bool,
}

/// Daily balance: Ist + Gutschrift − Soll.
pub fn day_balance_minutes(worked: i64, credited: i64, expected: i64) -> i64 {
    worked + credited - expected
}

fn default_true() -> bool {
    true
}

impl Default for SettlementRuleConfig {
    fn default() -> Self {
        Self {
            warn_negative_balance: true,
            enforce_flex_band: false,
        }
    }
}

pub fn parse_workday_config(json: &str) -> anyhow::Result<WorkdayModelConfig> {
    Ok(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_expected_part_time() {
        let cfg = WorkdayModelConfig {
            expected_minutes: 480,
            ..WorkdayModelConfig::default()
        };
        assert_eq!(crate::work_calendar::scale_expected(&cfg, 100), 480);
        assert_eq!(crate::work_calendar::scale_expected(&cfg, 50), 240);
        assert_eq!(crate::work_calendar::scale_expected(&cfg, 1), 4);
    }

    #[test]
    fn round_minutes_nearest_step() {
        assert_eq!(round_minutes_to_step(7, 5), 5);
        assert_eq!(round_minutes_to_step(8, 5), 10);
        assert_eq!(round_minutes_to_step(482, 15), 480);
    }

    #[test]
    fn flex_band_advisory_outside() {
        let cfg = WorkdayModelConfig::default();
        assert!(flex_band_advisory("05:30", &cfg).is_some());
        assert!(flex_band_advisory("20:30", &cfg).is_some());
        assert!(flex_band_advisory("10:00", &cfg).is_none());
    }

    #[test]
    fn day_balance_formula() {
        assert_eq!(day_balance_minutes(480, 0, 480), 0);
        assert_eq!(day_balance_minutes(420, 60, 480), 0);
        assert_eq!(day_balance_minutes(500, 0, 480), 20);
    }
}

pub fn standard_models() -> Vec<(&'static str, &'static str, WorkdayModelConfig)> {
    vec![
        (
            "wm-std-8h",
            "Standard 8h (Mo–Fr)",
            WorkdayModelConfig::default(),
        ),
        (
            "wm-rest",
            "Ruhetag",
            WorkdayModelConfig::rest_day(),
        ),
        (
            "wm-holiday-paid",
            "Feiertag bezahlt",
            WorkdayModelConfig::holiday_paid(),
        ),
        (
            "wm-short-6h",
            "Kurztag 6h",
            WorkdayModelConfig::short_day_6h(),
        ),
    ]
}
