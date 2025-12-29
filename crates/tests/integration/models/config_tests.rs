use systemprompt_models::config::Config;
use systemprompt_traits::validation_report::{ValidationError, ValidationReport, ValidationWarning};
use tempfile::TempDir;

#[test]
fn test_validate_required_path_exists() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_string_lossy().to_string();

    let mut report = ValidationReport::new("test");
    Config::validate_required_path(&mut report, "system", &path);

    assert!(!report.has_errors());
}

#[test]
fn test_validate_required_path_missing() {
    let mut report = ValidationReport::new("test");
    Config::validate_required_path(&mut report, "system", "/nonexistent/path");

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.system");
    assert!(report.errors[0].message.contains("does not exist"));
}

#[test]
fn test_validate_required_path_empty() {
    let mut report = ValidationReport::new("test");
    Config::validate_required_path(&mut report, "system", "");

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.system");
    assert!(report.errors[0].message.contains("not configured"));
}

#[test]
fn test_validate_required_optional_path_exists() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_string_lossy().to_string();

    let mut report = ValidationReport::new("test");
    Config::validate_required_optional_path(&mut report, "skills", &Some(path));

    assert!(!report.has_errors());
}

#[test]
fn test_validate_required_optional_path_none() {
    let mut report = ValidationReport::new("test");
    Config::validate_required_optional_path(&mut report, "skills", &None);

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.skills");
    assert!(report.errors[0].message.contains("not configured"));
}

#[test]
fn test_validate_required_optional_path_empty() {
    let mut report = ValidationReport::new("test");
    Config::validate_required_optional_path(&mut report, "skills", &Some(String::new()));

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.skills");
    assert!(report.errors[0].message.contains("empty"));
}

#[test]
fn test_validate_required_optional_path_missing() {
    let mut report = ValidationReport::new("test");
    Config::validate_required_optional_path(
        &mut report,
        "skills",
        &Some("/nonexistent/path".to_string()),
    );

    assert!(report.has_errors());
    assert_eq!(report.errors[0].field, "paths.skills");
    assert!(report.errors[0].message.contains("does not exist"));
}

#[test]
fn test_validate_optional_path_warns() {
    let mut report = ValidationReport::new("test");
    Config::validate_optional_path(
        &mut report,
        "geoip",
        &Some("/nonexistent/path".to_string()),
    );

    assert!(!report.has_errors());
    assert!(report.has_warnings());
    assert_eq!(report.warnings[0].field, "paths.geoip");
}

#[test]
fn test_validate_optional_path_none_no_warning() {
    let mut report = ValidationReport::new("test");
    Config::validate_optional_path(&mut report, "geoip", &None);

    assert!(!report.has_errors());
    assert!(!report.has_warnings());
}

#[test]
fn test_validate_optional_path_exists_no_warning() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_string_lossy().to_string();

    let mut report = ValidationReport::new("test");
    Config::validate_optional_path(&mut report, "geoip", &Some(path));

    assert!(!report.has_errors());
    assert!(!report.has_warnings());
}

#[test]
fn test_format_path_errors_includes_profile() {
    let mut report = ValidationReport::new("paths");
    report.add_error(
        ValidationError::new("paths.system", "Path does not exist")
            .with_path("/var/test")
            .with_suggestion("Create the path"),
    );

    let formatted = Config::format_path_errors(&report, "/path/to/profile.yaml");

    assert!(formatted.contains("Profile Path Validation Failed"));
    assert!(formatted.contains("/path/to/profile.yaml"));
    assert!(formatted.contains("paths.system"));
    assert!(formatted.contains("Path does not exist"));
    assert!(formatted.contains("/var/test"));
    assert!(formatted.contains("Create the path"));
}

#[test]
fn test_format_path_errors_includes_warnings() {
    let mut report = ValidationReport::new("paths");
    report.add_error(ValidationError::new("paths.system", "Missing"));
    report.add_warning(
        ValidationWarning::new("paths.geoip", "Path does not exist: /var/geoip")
            .with_suggestion("Create the path"),
    );

    let formatted = Config::format_path_errors(&report, "/profile.yaml");

    assert!(formatted.contains("ERRORS:"));
    assert!(formatted.contains("WARNINGS:"));
    assert!(formatted.contains("paths.geoip"));
}
