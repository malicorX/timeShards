use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AccessSchedule {
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub weekdays: Vec<u32>,
    pub start: String,
    pub end: String,
}

fn default_timezone() -> String {
    "Europe/Berlin".into()
}

/// `weekdays`: 1 = Monday … 7 = Sunday (ISO). Empty = all days.
pub fn schedule_allows(schedule_json: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(raw) = schedule_json else {
        return true;
    };
    if raw.trim().is_empty() {
        return true;
    }
    let Ok(schedule) = serde_json::from_str::<AccessSchedule>(raw) else {
        return true;
    };

    let tz: Tz = schedule.timezone.parse().unwrap_or(chrono_tz::Europe::Berlin);
    let local = now.with_timezone(&tz);
    let weekday = local.weekday().number_from_monday();
    if !schedule.weekdays.is_empty() && !schedule.weekdays.contains(&weekday) {
        return false;
    }

    let start_parts = parse_hm(&schedule.start);
    let end_parts = parse_hm(&schedule.end);
    let (Some((sh, sm)), Some((eh, em))) = (start_parts, end_parts) else {
        return true;
    };

    let minutes = local.hour() * 60 + local.minute();
    let start_m = sh * 60 + sm;
    let end_m = eh * 60 + em;
    if start_m <= end_m {
        minutes >= start_m && minutes < end_m
    } else {
        minutes >= start_m || minutes < end_m
    }
}

fn parse_hm(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.split(':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    Some((h, m))
}
