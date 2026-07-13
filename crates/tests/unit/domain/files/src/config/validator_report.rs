//! Report-level tests for `FilesConfigValidator::load` + `validate`.
//!
//! Each test runs in its own nextest process, bootstraps the tempdir-backed
//! profile, writes its own `services/config/files.yaml` variant, and asserts
//! the exact errors/warnings the validator reports for it.

use systemprompt_files::FilesConfigValidator;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

struct StubProvider;

impl ConfigProvider for StubProvider {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }

    fn database_url(&self) -> &str {
        "postgres://unused"
    }

    fn system_path(&self) -> &str {
        "/unused"
    }

    fn api_port(&self) -> u16 {
        0
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn write_files_yaml(content: &str) {
    let b = ensure_test_bootstrap();
    std::fs::write(b.services_path.join("config/files.yaml"), content).expect("write files.yaml");
}

fn loaded_validator() -> FilesConfigValidator {
    let mut v = FilesConfigValidator::new();
    v.load(&StubProvider).expect("load files.yaml");
    v
}

#[test]
fn default_config_validates_clean() {
    ensure_test_bootstrap();
    let v = loaded_validator();
    let report = v.validate().expect("validate");
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn url_prefix_without_leading_slash_is_an_error() {
    write_files_yaml("files:\n  urlPrefix: assets\n");
    let v = loaded_validator();
    let report = v.validate().expect("validate");
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].field, "files.urlPrefix");
    assert_eq!(report.errors[0].message, "URL prefix must start with '/'");
    assert!(report.warnings.is_empty());
}

#[test]
fn max_file_size_over_two_gb_warns() {
    write_files_yaml("files:\n  upload:\n    max_file_size_bytes: 3221225472\n");
    let v = loaded_validator();
    let report = v.validate().expect("validate");
    assert!(report.errors.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].field, "files.upload.maxFileSizeBytes");
    assert_eq!(
        report.warnings[0].message,
        "Max file size > 2GB may cause memory issues"
    );
    assert_eq!(
        report.warnings[0].suggestion.as_deref(),
        Some("Consider using a smaller max file size for better performance")
    );
}

#[test]
fn video_enabled_with_small_max_size_warns() {
    write_files_yaml(
        "files:\n  upload:\n    max_file_size_bytes: 1048576\n    allowed_types:\n      images: true\n      documents: true\n      audio: true\n      video: true\n",
    );
    let v = loaded_validator();
    let report = v.validate().expect("validate");
    assert!(report.errors.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].field, "files.upload.allowedTypes.video");
    assert_eq!(
        report.warnings[0].message,
        "Video uploads enabled but max file size < 100MB"
    );
    assert_eq!(
        report.warnings[0].suggestion.as_deref(),
        Some("Increase maxFileSizeBytes to at least 100MB for video uploads")
    );
}

#[test]
fn malformed_yaml_fails_load_with_parse_error() {
    write_files_yaml("files: [notamap\n");
    let mut v = FilesConfigValidator::new();
    let err = v.load(&StubProvider).expect_err("malformed yaml");
    match err {
        DomainConfigError::LoadError { message } => {
            assert!(
                message.contains("Failed to parse files.yaml"),
                "unexpected message: {message}"
            );
        },
        other => panic!("expected LoadError, got {other:?}"),
    }
}

#[test]
fn unreadable_files_yaml_fails_load_with_read_error() {
    let b = ensure_test_bootstrap();
    // A directory at the files.yaml path makes read_to_string fail while
    // `exists()` still passes, driving the read-error arm deterministically.
    std::fs::create_dir_all(b.services_path.join("config/files.yaml")).expect("mkdir files.yaml");
    let mut v = FilesConfigValidator::new();
    let err = v.load(&StubProvider).expect_err("unreadable yaml");
    match err {
        DomainConfigError::LoadError { message } => {
            assert!(
                message.contains("Failed to read files.yaml"),
                "unexpected message: {message}"
            );
        },
        other => panic!("expected LoadError, got {other:?}"),
    }
}
