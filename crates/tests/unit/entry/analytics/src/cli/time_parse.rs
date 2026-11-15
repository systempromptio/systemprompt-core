//! Tests for `analytics::shared::time` — relative-duration and
//! date/timestamp parsing for `--since`/`--until`, plus period truncation.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use systemprompt_cli::analytics::shared::{
    parse_duration, parse_since, parse_time_range, parse_until, truncate_to_period,
};

#[test]
fn parse_duration_recognises_each_unit_suffix() {
    assert_eq!(parse_duration("3d"), Some(Duration::days(3)));
    assert_eq!(parse_duration("12h"), Some(Duration::hours(12)));
    assert_eq!(parse_duration("30m"), Some(Duration::minutes(30)));
    assert_eq!(parse_duration("2w"), Some(Duration::weeks(2)));
}

#[test]
fn parse_duration_is_case_insensitive_and_trims() {
    assert_eq!(parse_duration("  24H "), Some(Duration::hours(24)));
}

#[test]
fn parse_duration_rejects_bare_numbers_and_garbage() {
    assert_eq!(parse_duration("7"), None);
    assert_eq!(parse_duration("abc"), None);
    assert_eq!(parse_duration("xh"), None);
}

#[test]
fn parse_since_none_input_yields_none() {
    assert!(parse_since(None).unwrap().is_none());
}

#[test]
fn parse_since_relative_duration_is_in_the_past() {
    let before = Utc::now() - Duration::hours(2);
    let parsed = parse_since(Some(&"1h".to_owned())).unwrap().unwrap();
    assert!(parsed > before);
    assert!(parsed <= Utc::now());
}

#[test]
fn parse_since_calendar_date_anchors_to_midnight() {
    let parsed = parse_since(Some(&"2026-01-13".to_owned()))
        .unwrap()
        .unwrap();
    assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 1, 13, 0, 0, 0).unwrap());
}

#[test]
fn parse_since_lowercases_input_so_timestamp_separator_is_rejected() {
    // `parse_since` lowercases the whole string before parsing, turning the
    // RFC-3339 `T` separator into `t`, which the `%Y-%m-%dT%H:%M:%S` format
    // refuses — so a literal timestamp does not round-trip here.
    let err = parse_since(Some(&"2026-01-13T10:30:45".to_owned())).unwrap_err();
    assert!(err.to_string().contains("Invalid --since format"));
}

#[test]
fn parse_since_rejects_unparseable_input() {
    let err = parse_since(Some(&"not-a-date".to_owned())).unwrap_err();
    assert!(err.to_string().contains("Invalid --since format"));
}

#[test]
fn parse_until_calendar_date_anchors_to_end_of_day() {
    let parsed = parse_until(Some(&"2026-01-13".to_owned()))
        .unwrap()
        .unwrap();
    assert_eq!(
        parsed,
        Utc.with_ymd_and_hms(2026, 1, 13, 23, 59, 59).unwrap()
    );
}

#[test]
fn parse_until_rejects_unparseable_input() {
    let err = parse_until(Some(&"garbage".to_owned())).unwrap_err();
    assert!(err.to_string().contains("Invalid --until format"));
}

#[test]
fn parse_time_range_defaults_to_last_24h_window() {
    let (start, end) = parse_time_range(None, None).unwrap();
    let span = end - start;
    assert!(span >= Duration::hours(23));
    assert!(span <= Duration::hours(25));
}

#[test]
fn parse_time_range_honours_explicit_bounds() {
    let (start, end) = parse_time_range(
        Some(&"2026-01-10".to_owned()),
        Some(&"2026-01-13".to_owned()),
    )
    .unwrap();
    assert_eq!(start, Utc.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap());
    assert_eq!(end, Utc.with_ymd_and_hms(2026, 1, 13, 23, 59, 59).unwrap());
}

#[test]
fn truncate_to_hour_zeroes_minutes_and_seconds() {
    let dt = Utc.with_ymd_and_hms(2026, 1, 13, 10, 45, 30).unwrap();
    let t = truncate_to_period(dt, "hour");
    assert_eq!(t.hour(), 10);
    assert_eq!(t.minute(), 0);
    assert_eq!(t.second(), 0);
}

#[test]
fn truncate_to_day_zeroes_time_of_day() {
    let dt = Utc.with_ymd_and_hms(2026, 1, 13, 10, 45, 30).unwrap();
    let t = truncate_to_period(dt, "day");
    assert_eq!(t, Utc.with_ymd_and_hms(2026, 1, 13, 0, 0, 0).unwrap());
}

#[test]
fn truncate_to_week_snaps_back_to_monday() {
    // 2026-01-14 is a Wednesday; Monday of that week is 2026-01-12.
    let dt = Utc.with_ymd_and_hms(2026, 1, 14, 9, 0, 0).unwrap();
    let t = truncate_to_period(dt, "week");
    assert_eq!(t, Utc.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap());
    assert_eq!(t.weekday(), chrono::Weekday::Mon);
}

#[test]
fn truncate_to_month_snaps_to_first_of_month() {
    let dt = Utc.with_ymd_and_hms(2026, 1, 14, 9, 0, 0).unwrap();
    let t = truncate_to_period(dt, "month");
    assert_eq!(t, Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
}

#[test]
fn truncate_to_unknown_period_is_identity() {
    let dt: DateTime<Utc> = Utc.with_ymd_and_hms(2026, 1, 14, 9, 13, 7).unwrap();
    assert_eq!(truncate_to_period(dt, "decade"), dt);
}
