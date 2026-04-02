use systemprompt_models::config::{
    format_path_errors, validate_optional_path, validate_postgres_url, validate_required_optional_path,
    validate_required_path,
};
use systemprompt_traits::validation_report::{ValidationReport, ValidationWarning};
use tempfile::TempDir;

#[test]
fn test_validate_required_path_empty_adds_error() {
    let mut report = ValidationReport::new("paths");
    validate_required_path(&mut report, "system", "");
    assert!(report.has_errors());
    assert_eq!(report.errors.len(), 1);
    assert!(report.errors[0].field.contains("system"));
}

#[test]
fn test_validate_required_path_nonexistent_adds_error() {
    let mut report = ValidationReport::new("paths");
    validate_required_path(&mut report, "services", "/nonexistent/path/12345");
    assert!(report.has_errors());
    assert!(report.errors[0].message.contains("does not exist"));
}

#[test]
fn test_validate_required_path_existing_no_error() {
    let temp_dir = TempDir::new().unwrap();
    let mut report = ValidationReport::new("paths");
    validate_required_path(&mut report, "system", temp_dir.path().to_str().unwrap());
    assert!(!report.has_errors());
}

#[test]
fn test_validate_optional_path_none_no_warning() {
    let mut report = ValidationReport::new("paths");
    validate_optional_path(&mut report, "geoip", None);
    assert!(!report.has_warnings());
}

#[test]
fn test_validate_optional_path_empty_no_warning() {
    let mut report = ValidationReport::new("paths");
    validate_optional_path(&mut report, "storage", Some(&String::new()));
    assert!(!report.has_warnings());
}

#[test]
fn test_validate_optional_path_nonexistent_adds_warning() {
    let mut report = ValidationReport::new("paths");
    let path = "/nonexistent/optional/path/12345".to_string();
    validate_optional_path(&mut report, "geoip", Some(&path));
    assert!(report.has_warnings());
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn test_validate_optional_path_existing_no_warning() {
    let temp_dir = TempDir::new().unwrap();
    let mut report = ValidationReport::new("paths");
    let path = temp_dir.path().to_str().unwrap().to_string();
    validate_optional_path(&mut report, "storage", Some(&path));
    assert!(!report.has_warnings());
}

#[test]
fn test_validate_required_optional_path_none_adds_error() {
    let mut report = ValidationReport::new("paths");
    validate_required_optional_path(&mut report, "bin", None);
    assert!(report.has_errors());
    assert!(report.errors[0].message.contains("not configured"));
}

#[test]
fn test_validate_required_optional_path_empty_adds_error() {
    let mut report = ValidationReport::new("paths");
    let empty = String::new();
    validate_required_optional_path(&mut report, "bin", Some(&empty));
    assert!(report.has_errors());
    assert!(report.errors[0].message.contains("empty"));
}

#[test]
fn test_validate_required_optional_path_nonexistent_adds_error() {
    let mut report = ValidationReport::new("paths");
    let path = "/nonexistent/required/optional/12345".to_string();
    validate_required_optional_path(&mut report, "bin", Some(&path));
    assert!(report.has_errors());
    assert!(report.errors[0].message.contains("does not exist"));
}

#[test]
fn test_validate_required_optional_path_existing_no_error() {
    let temp_dir = TempDir::new().unwrap();
    let mut report = ValidationReport::new("paths");
    let path = temp_dir.path().to_str().unwrap().to_string();
    validate_required_optional_path(&mut report, "bin", Some(&path));
    assert!(!report.has_errors());
}

#[test]
fn test_validate_postgres_url_valid_postgres() {
    let result = validate_postgres_url("postgres://localhost:5432/db");
    assert!(result.is_ok());
}

#[test]
fn test_validate_postgres_url_valid_postgresql() {
    let result = validate_postgres_url("postgresql://user:pass@host:5432/db");
    assert!(result.is_ok());
}

#[test]
fn test_validate_postgres_url_invalid_mysql() {
    let result = validate_postgres_url("mysql://localhost/db");
    assert!(result.is_err());
}

#[test]
fn test_validate_postgres_url_empty() {
    let result = validate_postgres_url("");
    assert!(result.is_err());
}

#[test]
fn test_validate_postgres_url_file_path() {
    let result = validate_postgres_url("/var/data/database.db");
    assert!(result.is_err());
}

#[test]
fn test_format_path_errors_contains_header() {
    let report = ValidationReport::new("paths");
    let output = format_path_errors(&report, "/config/profile.yaml");
    assert!(output.contains("Profile Path Validation Failed"));
    assert!(output.contains("/config/profile.yaml"));
}

#[test]
fn test_format_path_errors_includes_errors() {
    let mut report = ValidationReport::new("paths");
    report.add_error(
        systemprompt_traits::validation_report::ValidationError::new("paths.system", "Not found")
            .with_path("/etc/app/system")
            .with_suggestion("Create the directory"),
    );
    let output = format_path_errors(&report, "/config/profile.yaml");
    assert!(output.contains("[paths]"));
    assert!(output.contains("paths.system"));
    assert!(output.contains("Not found"));
    assert!(output.contains("Path:"));
    assert!(output.contains("To fix:"));
}

#[test]
fn test_format_path_errors_includes_warnings() {
    let mut report = ValidationReport::new("paths");
    report.add_warning(
        ValidationWarning::new("paths.geoip", "Optional path missing")
            .with_suggestion("Download the database"),
    );
    let output = format_path_errors(&report, "/config/profile.yaml");
    assert!(output.contains("WARNINGS:"));
    assert!(output.contains("paths.geoip"));
    assert!(output.contains("To enable:"));
}
