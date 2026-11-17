//! Unit tests for the LoggingExtension implementation.

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Extension, StandingTag};
use systemprompt_logging::LoggingExtension;
use systemprompt_test_fixtures::admitted_member;

fn contributions() -> ContributionSink {
    admitted_member(&LoggingExtension, StandingTag::Core)
        .expect("logging contributes")
        .sink
}

#[test]
fn metadata_id_and_name() {
    let m = LoggingExtension.metadata();
    assert_eq!(m.id, "logging");
    assert_eq!(m.name, "Logging");
    assert!(!m.version.is_empty());
}

#[test]
fn schemas_include_logs_and_analytics() {
    let sink = contributions();
    let names: Vec<&str> = sink
        .schemas()
        .iter()
        .filter_map(|s| s.table.as_deref())
        .collect();
    assert_eq!(
        names,
        vec![
            "logs",
            "analytics_events",
            "log_rollup_hourly",
            "analytics_link_click_rollup_hourly"
        ]
    );
}

#[test]
fn schemas_have_required_columns() {
    let sink = contributions();
    let schemas = sink.schemas();
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
fn no_reporting_capture_or_privacy_sql_is_registered() {
    assert!(
        contributions()
            .schemas()
            .iter()
            .all(|schema| !schema.sql.contains("reporting")),
        "the reporting projection is retired"
    );
}
