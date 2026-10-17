use chrono::{DateTime, TimeZone, Utc};
use systemprompt_gateway::{CalendarPeriod, QuotaWindow, WindowSpan};

fn at(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, s).unwrap()
}

fn month() -> QuotaWindow {
    QuotaWindow {
        calendar: Some(CalendarPeriod::Month),
        ..QuotaWindow::default()
    }
}

#[test]
fn calendar_month_starts_on_the_first_and_ends_on_the_next_first() {
    for day in [1, 15, 28, 31] {
        let span = month().span(at(2026, 1, day, 12, 0, 0));
        assert_eq!(span.start, at(2026, 1, 1, 0, 0, 0), "day {day}");
        assert_eq!(span.end, at(2026, 2, 1, 0, 0, 0), "day {day}");
        assert_eq!(span.bucket_seconds, 31 * 86_400);
    }
}

#[test]
fn calendar_month_length_follows_leap_years() {
    assert_eq!(
        month().span(at(2028, 2, 29, 23, 59, 59)).bucket_seconds,
        29 * 86_400
    );
    assert_eq!(
        month().span(at(2026, 2, 10, 0, 0, 0)).bucket_seconds,
        28 * 86_400
    );
    assert_eq!(
        month().span(at(2026, 4, 30, 0, 0, 0)).bucket_seconds,
        30 * 86_400
    );
}

#[test]
fn calendar_month_rolls_over_the_year() {
    let span = month().span(at(2026, 12, 31, 23, 59, 59));
    assert_eq!(span.start, at(2026, 12, 1, 0, 0, 0));
    assert_eq!(span.end, at(2027, 1, 1, 0, 0, 0));
}

#[test]
fn the_last_second_of_a_month_and_the_first_of_the_next_land_in_different_buckets() {
    let before = month().span(at(2026, 1, 31, 23, 59, 59));
    let after = month().span(at(2026, 2, 1, 0, 0, 1));
    assert_ne!(before.start, after.start);
    assert_eq!(before.end, after.start);
}

#[test]
fn calendar_week_starts_on_monday_utc() {
    let span = WindowSpan::calendar(at(2026, 10, 18, 22, 0, 0), CalendarPeriod::Week);
    assert_eq!(span.start, at(2026, 10, 12, 0, 0, 0));
    assert_eq!(span.end, at(2026, 10, 19, 0, 0, 0));
    assert_eq!(span.bucket_seconds, 7 * 86_400);
}

#[test]
fn the_first_instant_of_a_month_opens_the_new_bucket() {
    let span = month().span(at(2026, 3, 1, 0, 0, 0));
    assert_eq!(span.start, at(2026, 3, 1, 0, 0, 0));
    assert_eq!(span.end, at(2026, 4, 1, 0, 0, 0));
}

#[test]
fn sunday_midnight_closes_the_week_and_monday_opens_the_next() {
    let sunday = WindowSpan::calendar(at(2026, 10, 18, 23, 59, 59), CalendarPeriod::Week);
    let monday = WindowSpan::calendar(at(2026, 10, 19, 0, 0, 0), CalendarPeriod::Week);
    assert_eq!(sunday.end, monday.start);
    assert_eq!(monday.start, at(2026, 10, 19, 0, 0, 0));
    assert_eq!(monday.end, at(2026, 10, 26, 0, 0, 0));
}

#[test]
fn a_week_spanning_the_new_year_starts_in_the_old_year() {
    let span = WindowSpan::calendar(at(2027, 1, 1, 8, 0, 0), CalendarPeriod::Week);
    assert_eq!(span.start, at(2026, 12, 28, 0, 0, 0));
    assert_eq!(span.end, at(2027, 1, 4, 0, 0, 0));
}

#[test]
fn calendar_day_is_the_same_bucket_as_a_fixed_86400_window() {
    let now = at(2026, 10, 16, 9, 30, 0);
    let day = WindowSpan::calendar(now, CalendarPeriod::Day);
    let fixed = WindowSpan::fixed(now, 86_400);
    assert_eq!(day, fixed);
}

#[test]
fn a_fixed_window_aligns_to_the_epoch() {
    let span = WindowSpan::fixed(at(2026, 10, 16, 9, 30, 17), 60);
    assert_eq!(span.start, at(2026, 10, 16, 9, 30, 0));
    assert_eq!(span.end, at(2026, 10, 16, 9, 31, 0));
    assert_eq!(span.bucket_seconds, 60);
}

#[test]
fn calendar_month_parses_without_window_seconds() {
    let qw: QuotaWindow =
        serde_yaml::from_str("calendar: month\nsubject: user\nmax_cost_microdollars: 5000000")
            .unwrap();
    assert_eq!(qw.calendar, Some(CalendarPeriod::Month));
    assert_eq!(qw.window_seconds, 0);
    assert_eq!(qw.label(), "calendar-month");
}

#[test]
fn an_unknown_calendar_period_is_rejected() {
    assert!(serde_yaml::from_str::<QuotaWindow>("calendar: fortnight").is_err());
}

#[test]
fn a_fixed_window_serializes_without_a_calendar_key() {
    let yaml = serde_yaml::to_string(&QuotaWindow {
        window_seconds: 3600,
        ..QuotaWindow::default()
    })
    .unwrap();
    assert!(!yaml.contains("calendar"));
}
