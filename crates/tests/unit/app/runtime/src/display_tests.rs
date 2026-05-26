//! Exercises `display_validation_report` and `display_validation_warnings`
//! by constructing real `StartupValidationReport` payloads.
//! The functions write to CliService — we don't assert on output, just that
//! they execute every branch without panicking.

use std::path::PathBuf;
use systemprompt_runtime::{display_validation_report, display_validation_warnings};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{StartupValidationReport, ValidationReport, ValidationWarning};

fn make_error_report(domain: &str, with_path: bool, with_suggestion: bool) -> ValidationReport {
    let mut report = ValidationReport::new(domain);
    let mut err = ValidationError::new("field.x", "something is wrong");
    if with_path {
        err = err.with_path(PathBuf::from("/tmp/x.yaml"));
    }
    if with_suggestion {
        err = err.with_suggestion("set field.x to 'value'");
    }
    report.add_error(err);
    report
}

#[test]
fn display_empty_report_does_not_panic() {
    let report = StartupValidationReport::new();
    display_validation_report(&report);
}

#[test]
fn display_report_with_domain_errors_no_path_no_suggestion() {
    let mut report = StartupValidationReport::new();
    report.add_domain(make_error_report("config", false, false));
    display_validation_report(&report);
}

#[test]
fn display_report_with_domain_errors_full_metadata() {
    let mut report = StartupValidationReport::new();
    report.add_domain(make_error_report("config", true, true));
    display_validation_report(&report);
}

#[test]
fn display_report_with_extension_errors() {
    let mut report = StartupValidationReport::new();
    let mut ext = ValidationReport::new("ext-foo");
    ext.add_error(ValidationError::new("field", "boom"));
    report.add_extension(ext);
    display_validation_report(&report);
}

#[test]
fn display_report_with_profile_path() {
    let report =
        StartupValidationReport::new().with_profile_path(PathBuf::from("/profiles/local"));
    display_validation_report(&report);
}

#[test]
fn display_report_skips_clean_domain() {
    let mut report = StartupValidationReport::new();
    report.add_domain(ValidationReport::new("clean"));
    report.add_domain(make_error_report("dirty", false, false));
    display_validation_report(&report);
}

#[test]
fn display_report_skips_clean_extension() {
    let mut report = StartupValidationReport::new();
    report.add_extension(ValidationReport::new("clean-ext"));
    display_validation_report(&report);
}

#[test]
fn display_warnings_empty_is_noop() {
    let report = StartupValidationReport::new();
    display_validation_warnings(&report);
}

#[test]
fn display_warnings_with_simple_warning() {
    let mut report = StartupValidationReport::new();
    let mut d = ValidationReport::new("config");
    d.add_warning(ValidationWarning::new("field.x", "deprecated"));
    report.add_domain(d);
    display_validation_warnings(&report);
}

#[test]
fn display_warnings_with_suggestion() {
    let mut report = StartupValidationReport::new();
    let mut d = ValidationReport::new("config");
    d.add_warning(
        ValidationWarning::new("field.x", "deprecated").with_suggestion("use field.y"),
    );
    report.add_domain(d);
    display_validation_warnings(&report);
}

#[test]
fn display_warnings_multiple_domains() {
    let mut report = StartupValidationReport::new();
    for domain in ["config", "extensions", "templates"] {
        let mut d = ValidationReport::new(domain);
        d.add_warning(ValidationWarning::new("f", "msg"));
        d.add_warning(ValidationWarning::new("g", "msg2").with_suggestion("fix"));
        report.add_domain(d);
    }
    display_validation_warnings(&report);
}
