use std::str::FromStr;

use chrono::Utc;
use systemprompt_traits::events::{LogEventData, LogEventLevel};

#[test]
fn log_event_level_from_str_known_variants() {
    assert_eq!(LogEventLevel::from_str("error").unwrap(), LogEventLevel::Error);
    assert_eq!(LogEventLevel::from_str("warn").unwrap(), LogEventLevel::Warn);
    assert_eq!(
        LogEventLevel::from_str("warning").unwrap(),
        LogEventLevel::Warn
    );
    assert_eq!(LogEventLevel::from_str("info").unwrap(), LogEventLevel::Info);
    assert_eq!(
        LogEventLevel::from_str("debug").unwrap(),
        LogEventLevel::Debug
    );
    assert_eq!(
        LogEventLevel::from_str("trace").unwrap(),
        LogEventLevel::Trace
    );
}

#[test]
fn log_event_level_from_str_is_case_insensitive() {
    assert_eq!(LogEventLevel::from_str("ERROR").unwrap(), LogEventLevel::Error);
    assert_eq!(LogEventLevel::from_str("Info").unwrap(), LogEventLevel::Info);
}

#[test]
fn log_event_level_from_str_unknown_returns_error() {
    let err = LogEventLevel::from_str("nope").unwrap_err();
    assert!(err.contains("nope"));
}

#[test]
fn log_event_data_new_populates_fields() {
    let ts = Utc::now();
    let e = LogEventData::new(ts, LogEventLevel::Info, "auth", "user logged in");
    assert_eq!(e.timestamp, ts);
    assert_eq!(e.level, LogEventLevel::Info);
    assert_eq!(e.module, "auth");
    assert_eq!(e.message, "user logged in");
}
