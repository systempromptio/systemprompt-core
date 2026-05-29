//! Unit tests for the filesystem path-existence validators on
//! `systemprompt_config::path_validation`.

use systemprompt_config::path_validation::{
    format_path_errors, validate_optional_path, validate_required_path,
};
use systemprompt_traits::validation_report::{
    ValidationError, ValidationReport, ValidationWarning,
};
use tempfile::TempDir;

#[test]
fn validate_required_path_exists() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_string_lossy().to_string();

    let mut report = ValidationReport::new("test");
    validate_required_path(&mut report, "system", &path);

    assert!(!report.has_errors());
}

#[test]
fn validate_required_path_missing() {
    let mut report = ValidationReport::new("test");
    validate_required_path(&mut report, "system", "/nonexistent/path");

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.system");
    assert!(report.errors[0].message.contains("does not exist"));
}

#[test]
fn validate_required_path_empty() {
    let mut report = ValidationReport::new("test");
    validate_required_path(&mut report, "system", "");

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.system");
    assert!(report.errors[0].message.contains("not configured"));
}

#[test]
fn validate_optional_path_warns_on_missing() {
    let path = "/nonexistent/path".to_string();
    let mut report = ValidationReport::new("test");
    validate_optional_path(&mut report, "geoip", Some(&path));

    assert!(!report.has_errors());
    assert!(report.has_warnings());
    assert_eq!(report.warnings[0].field, "paths.geoip");
}

#[test]
fn validate_optional_path_none_no_warning() {
    let mut report = ValidationReport::new("test");
    validate_optional_path(&mut report, "geoip", None);

    assert!(!report.has_errors());
    assert!(!report.has_warnings());
}

#[test]
fn validate_optional_path_empty_no_warning() {
    let mut report = ValidationReport::new("test");
    validate_optional_path(&mut report, "storage", Some(&String::new()));

    assert!(!report.has_warnings());
}

#[test]
fn validate_optional_path_exists_no_warning() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_string_lossy().to_string();

    let mut report = ValidationReport::new("test");
    validate_optional_path(&mut report, "geoip", Some(&path));

    assert!(!report.has_errors());
    assert!(!report.has_warnings());
}

#[test]
fn format_path_errors_includes_profile_and_error_detail() {
    let mut report = ValidationReport::new("paths");
    report.add_error(
        ValidationError::new("paths.system", "Path does not exist")
            .with_path("/var/test")
            .with_suggestion("Create the path"),
    );

    let formatted = format_path_errors(&report, "/path/to/profile.yaml");

    assert!(formatted.contains("Profile Path Validation Failed"));
    assert!(formatted.contains("/path/to/profile.yaml"));
    assert!(formatted.contains("[paths]"));
    assert!(formatted.contains("paths.system"));
    assert!(formatted.contains("Path does not exist"));
    assert!(formatted.contains("Path:"));
    assert!(formatted.contains("To fix:"));
    assert!(formatted.contains("/var/test"));
}

#[test]
fn format_path_errors_includes_warnings() {
    let mut report = ValidationReport::new("paths");
    report.add_error(ValidationError::new("paths.system", "Missing"));
    report.add_warning(
        ValidationWarning::new("paths.geoip", "Path does not exist: /var/geoip")
            .with_suggestion("Download the database"),
    );

    let formatted = format_path_errors(&report, "/profile.yaml");

    assert!(formatted.contains("ERRORS:"));
    assert!(formatted.contains("WARNINGS:"));
    assert!(formatted.contains("paths.geoip"));
    assert!(formatted.contains("To enable:"));
}
