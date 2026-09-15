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
    let names: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();
    assert_eq!(names, vec!["logs", "analytics_events"]);
}

#[test]
fn schemas_have_required_columns() {
    let schemas = LoggingExtension.schemas();
    let logs = schemas
        .iter()
        .find(|s| s.table.as_deref() == Some("logs"))
        .unwrap();
    assert!(logs.required_columns.iter().any(|c| c == "id"));
    assert!(logs.required_columns.iter().any(|c| c == "level"));
    assert!(logs.required_columns.iter().any(|c| c == "timestamp"));

    let analytics = schemas
        .iter()
        .find(|s| s.table.as_deref() == Some("analytics_events"))
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

#[test]
fn owner_capture_and_privacy_contracts_are_registered() {
    let schemas = LoggingExtension.schemas();
    let capture: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_capture_reporting_change")
        })
        .collect();
    assert_eq!(
        capture.len(),
        1,
        "owner capture SQL must be registered exactly once"
    );
    for view in ["reporting_source_logs", "reporting_source_analytics_events"] {
        assert!(
            capture[0].sql.contains(view),
            "missing reporting view: {view}"
        );
    }
    let privacy: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION public.lock_logging_reporting_sources")
        })
        .collect();
    assert_eq!(
        privacy.len(),
        1,
        "owner privacy SQL must survive capture registration"
    );
}
