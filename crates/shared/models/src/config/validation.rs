use anyhow::Result;
use systemprompt_traits::validation_report::{
    ValidationError, ValidationReport, ValidationWarning,
};

use crate::profile::Profile;

pub fn validate_profile_paths(profile: &Profile, profile_path: &str) -> ValidationReport {
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

    validate_optional_path(&mut report, "geoip_database", &profile.paths.geoip_database);
    validate_optional_path(&mut report, "storage", &profile.paths.storage);

    let _ = profile_path;
    report
}

/// Validate a required path (must be set and exist).
pub fn validate_required_path(report: &mut ValidationReport, field: &str, path: &str) {
    if path.is_empty() {
        report.add_error(
            ValidationError::new(format!("paths.{}", field), "Required path not configured")
                .with_suggestion(format!(
                    "Add paths.{} to your profile or run 'systemprompt cloud config'",
                    field
                )),
        );
        return;
    }

    let path_buf = std::path::Path::new(path);
    if !path_buf.exists() {
        report.add_error(
            ValidationError::new(format!("paths.{}", field), "Path does not exist")
                .with_path(path)
                .with_suggestion("Create the directory/file or update the path in your profile"),
        );
    }
}

/// Validate a path that's Option<String> but required for operation.
pub fn validate_required_optional_path(
    report: &mut ValidationReport,
    field: &str,
    path: &Option<String>,
) {
    match path {
        None => {
            report.add_error(
                ValidationError::new(format!("paths.{}", field), "Required path not configured")
                    .with_suggestion(format!(
                        "Add paths.{} to your profile or run 'systemprompt cloud config'",
                        field
                    )),
            );
        },
        Some(p) if p.is_empty() => {
            report.add_error(
                ValidationError::new(format!("paths.{}", field), "Path is empty").with_suggestion(
                    format!("Set a valid path for paths.{} in your profile", field),
                ),
            );
        },
        Some(p) => {
            let path_buf = std::path::Path::new(p);
            if !path_buf.exists() {
                report.add_error(
                    ValidationError::new(format!("paths.{}", field), "Path does not exist")
                        .with_path(p)
                        .with_suggestion("Create the directory/file or update the path"),
                );
            }
        },
    }
}

/// Validate an optional path (warn if set but doesn't exist).
pub fn validate_optional_path(report: &mut ValidationReport, field: &str, path: &Option<String>) {
    if let Some(p) = path {
        if !p.is_empty() {
            let path_buf = std::path::Path::new(p);
            if !path_buf.exists() {
                report.add_warning(
                    ValidationWarning::new(
                        format!("paths.{}", field),
                        format!("Path does not exist: {}", p),
                    )
                    .with_suggestion("Create the path or remove the config entry"),
                );
            }
        }
    }
}

/// Format path validation errors for display.
pub fn format_path_errors(report: &ValidationReport, profile_path: &str) -> String {
    let mut output = String::new();
    output.push_str("Profile Path Validation Failed\n\n");
    output.push_str(&format!("Profile: {}\n\n", profile_path));
    output.push_str("ERRORS:\n\n");

    for error in &report.errors {
        output.push_str(&format!("[{}] {}\n", report.domain, error.field));
        output.push_str(&format!("  {}\n", error.message));
        if let Some(ref path) = error.path {
            output.push_str(&format!("  Path: {}\n", path.display()));
        }
        if let Some(ref suggestion) = error.suggestion {
            output.push_str(&format!("  To fix: {}\n", suggestion));
        }
        output.push('\n');
    }

    if !report.warnings.is_empty() {
        output.push_str("WARNINGS:\n\n");
        for warning in &report.warnings {
            output.push_str(&format!("[{}] {}\n", report.domain, warning.field));
            output.push_str(&format!("  {}\n", warning.message));
            if let Some(ref suggestion) = warning.suggestion {
                output.push_str(&format!("  To enable: {}\n", suggestion));
            }
            output.push('\n');
        }
    }

    output
}

/// Validate PostgreSQL connection URL.
pub fn validate_postgres_url(url: &str) -> Result<()> {
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err(anyhow::anyhow!(
            "DATABASE_URL must be a PostgreSQL connection string (postgres:// or postgresql://)"
        ));
    }
    Ok(())
}
