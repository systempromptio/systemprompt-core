//! Tests for `infra logs duration` and `infra logs shared` pure helpers — the
//! `--since` duration grammar (which, unlike the analytics variant, treats a
//! bare integer as a day count and surfaces a typed error) plus the
//! microdollar→dollar cost conversion.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{Duration, TimeZone, Utc};
use systemprompt_cli::infrastructure::logs::duration::{parse_duration, parse_since};
use systemprompt_cli::infrastructure::logs::shared::cost_microdollars_to_dollars;

#[test]
fn parse_duration_recognises_day_hour_minute_suffixes() {
    assert_eq!(parse_duration("7d").unwrap(), Duration::days(7));
    assert_eq!(parse_duration("24h").unwrap(), Duration::hours(24));
    assert_eq!(parse_duration("30m").unwrap(), Duration::minutes(30));
}

#[test]
fn parse_duration_treats_bare_integer_as_days() {
    assert_eq!(parse_duration("5").unwrap(), Duration::days(5));
}

#[test]
fn parse_duration_is_case_insensitive_and_trims() {
    assert_eq!(parse_duration("  12H ").unwrap(), Duration::hours(12));
}

#[test]
fn parse_duration_rejects_non_numeric_unit() {
    let err = parse_duration("xd").unwrap_err();
    assert!(err.to_string().contains("Invalid duration"));
}

#[test]
fn parse_duration_rejects_unknown_grammar() {
    let err = parse_duration("nonsense!").unwrap_err();
    assert!(err.to_string().contains("Invalid duration format"));
}

#[test]
fn parse_since_none_yields_none() {
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
fn parse_since_accepts_calendar_date_at_midnight() {
    let parsed = parse_since(Some(&"2026-01-13".to_owned()))
        .unwrap()
        .unwrap();
    assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 1, 13, 0, 0, 0).unwrap());
}

#[test]
fn parse_since_lowercases_input_so_timestamp_separator_is_rejected() {
    // The input is lowercased before parsing, turning the RFC-3339 `T`
    // separator into `t`, which the `%Y-%m-%dT%H:%M:%S` format rejects — a
    // literal timestamp therefore falls through to the error arm.
    let err = parse_since(Some(&"2026-01-13T08:15:00".to_owned())).unwrap_err();
    assert!(err.to_string().contains("Invalid --since format"));
}

#[test]
fn parse_since_rejects_unparseable_input() {
    let err = parse_since(Some(&"definitely not a date".to_owned())).unwrap_err();
    assert!(err.to_string().contains("Invalid --since format"));
}

#[test]
fn cost_microdollars_converts_to_dollars() {
    assert!((cost_microdollars_to_dollars(0) - 0.0).abs() < f64::EPSILON);
    assert!((cost_microdollars_to_dollars(1_000_000) - 1.0).abs() < f64::EPSILON);
    assert!((cost_microdollars_to_dollars(2_500_000) - 2.5).abs() < f64::EPSILON);
}
