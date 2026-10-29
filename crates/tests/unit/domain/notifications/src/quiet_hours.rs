use chrono::{NaiveTime, Weekday};
use systemprompt_models::notifications::QuietHours;
use systemprompt_notifications::pipeline::rules::in_quiet_hours;

use crate::fixtures::utc;

fn window(start: (u32, u32), end: (u32, u32), tz: &str) -> QuietHours {
    QuietHours {
        start: NaiveTime::from_hms_opt(start.0, start.1, 0).expect("time"),
        end: NaiveTime::from_hms_opt(end.0, end.1, 0).expect("time"),
        tz: tz.to_owned(),
        weekdays: None,
    }
}

#[test]
fn a_same_day_window_includes_its_start_and_excludes_its_end() {
    let lunch = window((12, 0), (13, 0), "UTC");
    assert!(in_quiet_hours(&lunch, utc(2026, 10, 29, 12, 0)));
    assert!(in_quiet_hours(&lunch, utc(2026, 10, 29, 12, 59)));
    assert!(!in_quiet_hours(&lunch, utc(2026, 10, 29, 13, 0)));
    assert!(!in_quiet_hours(&lunch, utc(2026, 10, 29, 11, 59)));
}

#[test]
fn an_overnight_window_wraps_midnight() {
    let night = window((22, 0), (7, 0), "Z");
    assert!(in_quiet_hours(&night, utc(2026, 10, 29, 23, 0)));
    assert!(in_quiet_hours(&night, utc(2026, 10, 30, 3, 0)));
    assert!(!in_quiet_hours(&night, utc(2026, 10, 30, 7, 0)));
    assert!(!in_quiet_hours(&night, utc(2026, 10, 29, 15, 0)));
}

#[test]
fn a_negative_offset_shifts_the_window() {
    let night = window((22, 0), (7, 0), "-05:00");
    assert!(
        in_quiet_hours(&night, utc(2026, 10, 30, 3, 30)),
        "22:30 local"
    );
    assert!(
        !in_quiet_hours(&night, utc(2026, 10, 29, 23, 0)),
        "18:00 local"
    );
}

#[test]
fn malformed_offsets_are_treated_as_inside_the_window() {
    for tz in ["+25:00", "+01", "01:00", "+aa:bb", "", "America/New_York"] {
        let lunch = window((12, 0), (13, 0), tz);
        assert!(
            in_quiet_hours(&lunch, utc(2026, 10, 29, 18, 0)),
            "{tz:?} must fail quiet"
        );
    }
}

#[test]
fn weekdays_restrict_the_window_in_local_time() {
    let mut weekend = window((0, 0), (23, 59), "+01:00");
    weekend.weekdays = Some(vec![Weekday::Sat, Weekday::Sun]);
    assert!(
        !in_quiet_hours(&weekend, utc(2026, 10, 30, 12, 0)),
        "a Friday is not quiet"
    );
    assert!(
        in_quiet_hours(&weekend, utc(2026, 10, 31, 12, 0)),
        "a Saturday is quiet"
    );
    assert!(
        in_quiet_hours(&weekend, utc(2026, 10, 30, 23, 30)),
        "23:30 UTC on Friday is already Saturday at +01:00"
    );
}
