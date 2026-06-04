//! Filesystem existence checks for the paths a profile declares.
//!
//! Structural profile validation (non-empty, cloud `/app` prefixes, security,
//! CORS, rate limits) is pure and lives in `systemprompt_models`. The checks
//! here touch the filesystem — confirming declared and derived paths actually
//! exist — so they belong in the infrastructure layer, at the config-build
//! boundary, not in the model layer.

use systemprompt_models::profile::Profile;
use systemprompt_traits::validation_report::{
    ValidationError, ValidationReport, ValidationWarning,
};

#[must_use]
pub fn validate_profile_paths(profile: &Profile, _profile_path: &str) -> ValidationReport {
    let mut report = ValidationReport::new("paths");

    validate_required_path(&mut report, "system", &profile.paths.system);
    validate_required_path(&mut report, "services", &profile.paths.services);
    validate_required_path(&mut report, "bin", &profile.paths.bin);

    validate_required_path(&mut report, "skills", &profile.paths.skills());
    validate_required_path(&mut report, "config", &profile.paths.config());
    validate_required_path(&mut report, "web_path", &profile.paths.web_path_resolved());
    validate_required_path(&mut report, "web_config", &profile.paths.web_config());
    validate_required_path(&mut report, "web_metadata", &profile.paths.web_metadata());
    validate_required_path(
        &mut report,
        "content_config",
        &profile.paths.content_config(),
    );

    validate_optional_path(
        &mut report,
        "geoip_database",
        profile.paths.geoip_database.as_ref(),
    );
    validate_optional_path(&mut report, "storage", profile.paths.storage.as_ref());

    report
}

pub fn validate_required_path(report: &mut ValidationReport, field: &str, path: &str) {
    if path.is_empty() {
        report.add_error(
            ValidationError::new(format!("paths.{field}"), "Required path not configured")
                .with_suggestion(format!(
                    "Add paths.{field} to your profile or run 'systemprompt cloud config'"
                )),
        );
        return;
    }

    if !std::path::Path::new(path).exists() {
        report.add_error(
            ValidationError::new(format!("paths.{field}"), "Path does not exist")
                .with_path(path)
                .with_suggestion("Create the directory/file or update the path in your profile"),
        );
    }
}

pub fn validate_optional_path(report: &mut ValidationReport, field: &str, path: Option<&String>) {
    if let Some(p) = path {
        if !p.is_empty() && !std::path::Path::new(p).exists() {
            report.add_warning(
                ValidationWarning::new(
                    format!("paths.{field}"),
                    format!("Path does not exist: {p}"),
                )
                .with_suggestion("Create the path or remove the config entry"),
            );
        }
    }
}

#[must_use]
pub fn format_path_errors(report: &ValidationReport, profile_path: &str) -> String {
    let mut output = String::new();
    output.push_str("Profile Path Validation Failed\n\n");
    output.push_str(&format!("Profile: {profile_path}\n\n"));
    output.push_str("ERRORS:\n\n");

    for error in &report.errors {
        output.push_str(&format!("[{}] {}\n", report.domain, error.field));
        output.push_str(&format!("  {}\n", error.message));
        if let Some(ref path) = error.path {
            output.push_str(&format!("  Path: {}\n", path.display()));
        }
        if let Some(ref suggestion) = error.suggestion {
            output.push_str(&format!("  To fix: {suggestion}\n"));
        }
        output.push('\n');
    }

    if !report.warnings.is_empty() {
        output.push_str("WARNINGS:\n\n");
        for warning in &report.warnings {
            output.push_str(&format!("[{}] {}\n", report.domain, warning.field));
            output.push_str(&format!("  {}\n", warning.message));
            if let Some(ref suggestion) = warning.suggestion {
                output.push_str(&format!("  To enable: {suggestion}\n"));
            }
            output.push('\n');
        }
    }

    output
}
