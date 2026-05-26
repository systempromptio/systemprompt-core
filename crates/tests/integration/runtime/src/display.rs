//! Drives the public `display_validation_report` and
//! `display_validation_warnings` helpers across error, warning, and
//! extension branches. Output goes through `CliService` (stderr); the
//! tests assert only that the helpers do not panic and that the
//! underlying report shape is what the helpers consume.

use std::path::PathBuf;

use systemprompt_runtime::{display_validation_report, display_validation_warnings};
use systemprompt_traits::validation_report::{ValidationError, ValidationWarning};
use systemprompt_traits::{StartupValidationReport, ValidationReport};

fn report_with_errors() -> StartupValidationReport {
    let mut report = StartupValidationReport::new().with_profile_path(PathBuf::from(
        "/tmp/integration-test/profile.yaml",
    ));

    let mut domain = ValidationReport::new("web");
    domain.add_error(
        ValidationError::new("web.host", "invalid host")
            .with_path(PathBuf::from("/tmp/web.yaml"))
            .with_suggestion("set web.host to a valid hostname"),
    );
    domain.add_warning(ValidationWarning::new("web.port", "non-standard port"));
    report.add_domain(domain);

    let mut ext_report = ValidationReport::new("ext:demo");
    ext_report.add_error(ValidationError::new(
        "ext_config.api_key",
        "extension api_key missing",
    ));
    report.add_extension(ext_report);

    report
}

fn report_with_warnings_only() -> StartupValidationReport {
    let mut report = StartupValidationReport::new();
    let mut domain = ValidationReport::new("content");
    domain.add_warning(
        ValidationWarning::new("content.sitemap", "no sitemap configured")
            .with_suggestion("add a sitemap entry under content_sources"),
    );
    report.add_domain(domain);
    report
}

#[test]
fn display_validation_report_with_errors_does_not_panic() {
    let report = report_with_errors();
    assert!(report.has_errors(), "fixture must report errors");
    assert!(report.error_count() >= 2, "two error sources expected");
    display_validation_report(&report);
}

#[test]
fn display_validation_warnings_with_warnings_does_not_panic() {
    let report = report_with_warnings_only();
    assert!(report.has_warnings(), "fixture must report warnings");
    assert!(!report.has_errors(), "fixture must be error-free");
    display_validation_warnings(&report);
}

#[test]
fn display_handles_empty_report() {
    let report = StartupValidationReport::new();
    display_validation_report(&report);
    display_validation_warnings(&report);
}

#[test]
fn display_handles_report_with_profile_path_only() {
    let report =
        StartupValidationReport::new().with_profile_path(PathBuf::from("/etc/sp/profile.yaml"));
    display_validation_report(&report);
}
