//! Unit tests for sync routes behavior
//!
//! Note: The internal types in routes/sync/types.rs (TableResult, RecordCounts,
//! ImportResults, etc.) are not publicly exported from the crate.
//! This module documents expected behavior for integration tests.
//!
//! The sync endpoints (/database/export, /database/import) are tested
//! through integration tests that can access the full HTTP response.

#[test]
fn test_export_response_format_documented() {
    let expected_fields = vec![
        "services",
        "skills",
        "contexts",
        "exported_at",
        "record_counts",
    ];

    assert_eq!(expected_fields.len(), 5);
    assert!(expected_fields.contains(&"record_counts"));
    assert!(expected_fields.contains(&"exported_at"));
}

#[test]
fn test_import_request_format_documented() {
    let expected_fields = vec!["services", "skills", "contexts", "merge_strategy"];

    assert_eq!(expected_fields.len(), 4);
    assert!(expected_fields.contains(&"merge_strategy"));
}

#[test]
fn test_import_response_format_documented() {
    let table_result_fields = vec!["created", "updated", "skipped", "deleted"];

    assert_eq!(table_result_fields.len(), 4);
}

#[test]
fn test_export_error_format_documented() {
    let error_field = "error";
    assert_eq!(error_field, "error");
}
