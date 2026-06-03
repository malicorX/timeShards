//! Calendar weeks align with Europe/Berlin (not UTC midnight).

use chrono::{TimeZone, Utc};
use timeshards_db::{week_bounds_utc, week_dates};

#[test]
fn week_start_is_berlin_monday_midnight() {
    // 2025-01-05 23:00 UTC = 2025-01-06 00:00 CET (Monday) in Berlin
    let now = Utc.with_ymd_and_hms(2025, 1, 5, 23, 0, 0).unwrap();
    let (start, end) = week_bounds_utc(now);
    assert_eq!(start, Utc.with_ymd_and_hms(2025, 1, 5, 23, 0, 0).unwrap());
    assert_eq!(end, Utc.with_ymd_and_hms(2025, 1, 12, 23, 0, 0).unwrap());
}

#[test]
fn week_dates_follow_berlin_monday() {
    let (start, _) = week_bounds_utc(Utc.with_ymd_and_hms(2025, 1, 5, 23, 0, 0).unwrap());
    let dates = week_dates(start);
    assert_eq!(dates[0].to_string(), "2025-01-06");
    assert_eq!(dates[6].to_string(), "2025-01-12");
}

#[test]
fn sunday_evening_berlin_still_same_week() {
    // 2025-01-12 22:30 UTC = Sunday 23:30 Berlin — still week starting 2025-01-06
    let now = Utc.with_ymd_and_hms(2025, 1, 12, 22, 30, 0).unwrap();
    let (start, _) = week_bounds_utc(now);
    assert_eq!(start, Utc.with_ymd_and_hms(2025, 1, 5, 23, 0, 0).unwrap());
}
