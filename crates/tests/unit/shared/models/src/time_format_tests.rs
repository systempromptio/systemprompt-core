use chrono::{TimeZone, Utc};
use systemprompt_models::time_format::{
    format_date, format_date_range, format_duration_ms, format_optional_duration_ms,
    format_period_label, format_timestamp,
};

fn dt(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, h, m, s).unwrap()
}

#[test]
fn format_timestamp_produces_datetime_string() {
    let d = dt(2025, 6, 15, 14, 30, 5);
    assert_eq!(format_timestamp(d), "2025-06-15 14:30:05");
}

#[test]
fn format_date_produces_date_only() {
    let d = dt(2025, 1, 7, 23, 59, 59);
    assert_eq!(format_date(d), "2025-01-07");
}

#[test]
fn format_date_range_combines_two_dates() {
    let start = dt(2025, 3, 1, 0, 0, 0);
    let end = dt(2025, 3, 7, 0, 0, 0);
    assert_eq!(format_date_range(start, end), "2025-03-01 to 2025-03-07");
}

#[test]
fn format_date_range_same_day() {
    let d = dt(2025, 12, 31, 0, 0, 0);
    assert_eq!(format_date_range(d, d), "2025-12-31 to 2025-12-31");
}

#[test]
fn format_duration_ms_below_1000_shows_ms() {
    assert_eq!(format_duration_ms(0), "0ms");
    assert_eq!(format_duration_ms(500), "500ms");
    assert_eq!(format_duration_ms(999), "999ms");
}

#[test]
fn format_duration_ms_seconds_range() {
    assert_eq!(format_duration_ms(1000), "1.0s");
    assert_eq!(format_duration_ms(1500), "1.5s");
    assert_eq!(format_duration_ms(59_999), "60.0s");
}

#[test]
fn format_duration_ms_minutes_range() {
    assert_eq!(format_duration_ms(60_000), "1.0m");
    assert_eq!(format_duration_ms(90_000), "1.5m");
    assert_eq!(format_duration_ms(3_599_999), "60.0m");
}

#[test]
fn format_duration_ms_hours_range() {
    assert_eq!(format_duration_ms(3_600_000), "1.0h");
    assert_eq!(format_duration_ms(7_200_000), "2.0h");
}

#[test]
fn format_optional_duration_ms_none_returns_empty() {
    assert_eq!(format_optional_duration_ms(None), "");
}

#[test]
fn format_optional_duration_ms_some_wraps_value() {
    assert_eq!(format_optional_duration_ms(Some(500)), " (500ms)");
    assert_eq!(format_optional_duration_ms(Some(2000)), " (2000ms)");
}

#[test]
fn format_period_label_hour() {
    let d = dt(2025, 4, 10, 15, 45, 0);
    assert_eq!(format_period_label(d, "hour"), "2025-04-10 15:00");
}

#[test]
fn format_period_label_day() {
    let d = dt(2025, 4, 10, 15, 45, 0);
    assert_eq!(format_period_label(d, "day"), "2025-04-10");
}

#[test]
fn format_period_label_week() {
    let d = dt(2025, 4, 7, 0, 0, 0);
    assert_eq!(format_period_label(d, "week"), "Week of 2025-04-07");
}

#[test]
fn format_period_label_month() {
    let d = dt(2025, 4, 10, 0, 0, 0);
    assert_eq!(format_period_label(d, "month"), "2025-04");
}

#[test]
fn format_period_label_unknown_falls_back_to_timestamp() {
    let d = dt(2025, 4, 10, 15, 45, 0);
    assert_eq!(format_period_label(d, "quarter"), "2025-04-10 15:45:00");
}
