//! Unit tests for the LoggingExtension implementation.

use systemprompt_extension::Extension;
use systemprompt_logging::LoggingExtension;

#[test]
fn metadata_id_and_name() {
    let m = LoggingExtension.metadata();
    assert_eq!(m.id, "logging");
    assert_eq!(m.name, "Logging");
    assert!(!m.version.is_empty());
}

#[test]
fn extension_is_required() {
    assert!(LoggingExtension.is_required());
}

#[test]
fn schemas_include_logs_and_analytics() {
    let schemas = LoggingExtension.schemas();
    let names: Vec<&str> = schemas.iter().map(|s| s.table.as_str()).collect();
    assert!(names.contains(&"logs"));
    assert!(names.contains(&"analytics_events"));
}

#[test]
fn schemas_have_required_columns() {
    let schemas = LoggingExtension.schemas();
    let logs = schemas.iter().find(|s| s.table == "logs").unwrap();
    assert!(logs.required_columns.iter().any(|c| c == "id"));
    assert!(logs.required_columns.iter().any(|c| c == "level"));
    assert!(logs.required_columns.iter().any(|c| c == "timestamp"));

    let analytics = schemas
        .iter()
        .find(|s| s.table == "analytics_events")
        .unwrap();
    assert!(analytics.required_columns.iter().any(|c| c == "id"));
    assert!(analytics.required_columns.iter().any(|c| c == "user_id"));
    assert!(analytics.required_columns.iter().any(|c| c == "severity"));
}

#[test]
fn dependencies_include_database_and_users() {
    let deps = LoggingExtension.dependencies();
    assert!(deps.contains(&"database"));
    assert!(deps.contains(&"users"));
}

#[test]
fn extension_default_constructs() {
    let _e = LoggingExtension::default();
}

#[test]
fn extension_copy_clone() {
    let e = LoggingExtension;
    let _e2 = e;
    let _e3 = e.clone();
    let _ = format!("{:?}", e);
}

#[test]
fn migrations_returns_vec() {
    let _ = LoggingExtension.migrations();
}
