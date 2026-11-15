//! Tests for the remaining arms of the `--since` / `--until` parsers.
//!
//! `time_parse` covers the duration shorthands; these drive the bare-date and
//! full-timestamp arms, which differ between the two parsers, plus the
//! period truncation helper.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{Datelike, TimeZone, Timelike, Utc};
use systemprompt_cli::analytics::shared::time::{
    parse_since, parse_time_range, parse_until, truncate_to_period,
};

fn arg(s: &str) -> String {
    s.to_owned()
}

#[test]
fn a_bare_date_starts_the_since_window_at_midnight() {
    let parsed = parse_since(Some(&arg("2026-01-13"))).unwrap().unwrap();

    assert_eq!(parsed.year(), 2026);
    assert_eq!(parsed.month(), 1);
    assert_eq!(parsed.day(), 13);
    assert_eq!((parsed.hour(), parsed.minute(), parsed.second()), (0, 0, 0));
}

#[test]
fn a_bare_date_ends_the_until_window_at_the_last_second() {
    let parsed = parse_until(Some(&arg("2026-01-13"))).unwrap().unwrap();

    assert_eq!(parsed.day(), 13);
    assert_eq!(
        (parsed.hour(), parsed.minute(), parsed.second()),
        (23, 59, 59)
    );
}

#[test]
fn a_full_timestamp_is_rejected_despite_being_the_advertised_format() {
    // Both parsers lowercase their input before matching `%Y-%m-%dT%H:%M:%S`,
    // which needs a literal uppercase `T`, so the timestamp form the error
    // message advertises never parses.
    let since = parse_since(Some(&arg("2026-01-13T10:30:00"))).unwrap_err();
    assert!(format!("{since:#}").contains("Invalid --since format"));

    let until = parse_until(Some(&arg("2026-01-13T10:30:00"))).unwrap_err();
    assert!(format!("{until:#}").contains("Invalid --until format"));
}

#[test]
fn surrounding_whitespace_is_tolerated_on_a_bare_date() {
    let parsed = parse_since(Some(&arg("  2026-01-13  "))).unwrap().unwrap();

    assert_eq!(parsed.day(), 13);
    assert_eq!(parsed.hour(), 0);
}

#[test]
fn an_unparseable_value_names_the_accepted_formats() {
    let since = parse_since(Some(&arg("last tuesday"))).unwrap_err();
    assert!(format!("{since:#}").contains("Invalid --since format"));
    assert!(format!("{since:#}").contains("2026-01-13"));

    let until = parse_until(Some(&arg("last tuesday"))).unwrap_err();
    assert!(format!("{until:#}").contains("Invalid --until format"));
}

#[test]
fn an_absent_bound_parses_to_no_constraint() {
    assert!(parse_since(None).unwrap().is_none());
    assert!(parse_until(None).unwrap().is_none());
}

#[test]
fn a_range_defaults_its_bounds_and_honours_explicit_ones() {
    let (start, end) = parse_time_range(None, None).unwrap();
    assert!(start < end, "{start} !< {end}");

    let (start, end) =
        parse_time_range(Some(&arg("2026-01-13")), Some(&arg("2026-01-14"))).unwrap();
    assert_eq!(start.day(), 13);
    assert_eq!(end.day(), 14);
    assert!(start < end);
}

#[test]
fn truncation_snaps_a_timestamp_to_the_start_of_its_period() {
    let dt = Utc.with_ymd_and_hms(2026, 1, 13, 10, 30, 45).unwrap();

    let hour = truncate_to_period(dt, "hour");
    assert_eq!((hour.hour(), hour.minute(), hour.second()), (10, 0, 0));

    let day = truncate_to_period(dt, "day");
    assert_eq!((day.hour(), day.minute(), day.second()), (0, 0, 0));
    assert_eq!(day.day(), 13);

    let week = truncate_to_period(dt, "week");
    assert_eq!((week.hour(), week.minute(), week.second()), (0, 0, 0));
    assert!(week <= day, "the week start cannot follow the day start");

    let month = truncate_to_period(dt, "month");
    assert_eq!(month.day(), 1);
    assert_eq!((month.hour(), month.minute(), month.second()), (0, 0, 0));

    // An unrecognised period leaves the timestamp untouched.
    assert_eq!(truncate_to_period(dt, "fortnight"), dt);
}
