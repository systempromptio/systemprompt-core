//! Additional tests for LoggingError variants not in log_error.rs:
//! PoolUnavailable, TaskNotFound, debug format, and the upstream conversions.

use systemprompt_logging::models::LoggingError;

#[test]
fn pool_unavailable_display() {
    let e = LoggingError::PoolUnavailable("no connections left".to_owned());
    assert_eq!(
        e.to_string(),
        "Database pool unavailable: no connections left"
    );
}

#[test]
fn pool_unavailable_debug() {
    let e = LoggingError::PoolUnavailable("err".to_owned());
    assert!(format!("{e:?}").contains("PoolUnavailable"));
}

#[test]
fn task_not_found_display() {
    let e = LoggingError::TaskNotFound {
        partial_id: "abc-123".to_owned(),
    };
    assert_eq!(e.to_string(), "No task found matching: abc-123");
}

#[test]
fn task_not_found_debug() {
    let e = LoggingError::TaskNotFound {
        partial_id: "xyz".to_owned(),
    };
    assert!(format!("{e:?}").contains("TaskNotFound"));
    assert!(format!("{e:?}").contains("xyz"));
}

#[test]
fn logging_error_from_json_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("not-json").unwrap_err();
    let e = LoggingError::from(json_err);
    assert!(e.to_string().contains("serialization"));
}

#[test]
fn logging_error_from_uuid_error() {
    let uuid_err = "not-a-uuid".parse::<uuid::Uuid>().unwrap_err();
    let e = LoggingError::from(uuid_err);
    assert!(e.to_string().contains("UUID"));
}

#[test]
fn logging_error_from_chrono_parse_error() {
    let dt_err = chrono::DateTime::parse_from_rfc3339("bad-date").unwrap_err();
    let e = LoggingError::from(dt_err);
    assert!(e.to_string().contains("DateTime"));
}
