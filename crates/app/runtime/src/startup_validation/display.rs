use systemprompt_logging::CliService;
use systemprompt_logging::services::cli::BrandColors;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

pub fn display_validation_report(report: &StartupValidationReport) {
    CliService::output("");
    CliService::output(&format!(
        "{} {}",
        BrandColors::stopped("✗"),
        BrandColors::white_bold("Validation Failed")
    ));

    if let Some(ref path) = report.profile_path {
        CliService::output(&format!(
            "  {} {}",
            BrandColors::dim("Profile:"),
            BrandColors::highlight(&path.display().to_string())
        ));
    }

    CliService::output("");
    CliService::output(&format!(
        "  {} error(s) found:",
        BrandColors::stopped(&report.error_count().to_string())
    ));

    for domain in &report.domains {
        display_domain_errors(domain);
    }

    for ext in &report.extensions {
        display_extension_errors(ext);
    }

    CliService::output("");
}

fn display_domain_errors(domain: &ValidationReport) {
    if !domain.has_errors() {
        return;
    }

    CliService::output("");
    CliService::output(&format!(
        "  {} {}",
        BrandColors::stopped("▸"),
        BrandColors::white_bold(&domain.domain)
    ));

    for error in &domain.errors {
        CliService::output(&format!(
            "    {} {}",
            BrandColors::dim("field:"),
            error.field
        ));
        CliService::output(&format!(
            "    {} {}",
            BrandColors::dim("error:"),
            error.message
        ));
        if let Some(ref path) = error.path {
            CliService::output(&format!(
                "    {} {}",
                BrandColors::dim("path:"),
                path.display()
            ));
        }
        if let Some(ref suggestion) = error.suggestion {
            CliService::output(&format!(
                "    {} {}",
                BrandColors::highlight("fix:"),
                suggestion
            ));
        }
    }
}

fn display_extension_errors(ext: &ValidationReport) {
    if !ext.has_errors() {
        return;
    }

    CliService::output("");
    CliService::output(&format!(
        "  {} {}",
        BrandColors::stopped("▸"),
        BrandColors::white_bold(&ext.domain)
    ));

    for error in &ext.errors {
        CliService::output(&format!(
            "    {} {}",
            BrandColors::dim("field:"),
            error.field
        ));
        CliService::output(&format!(
            "    {} {}",
            BrandColors::dim("error:"),
            error.message
        ));
    }
}

pub fn display_validation_warnings(report: &StartupValidationReport) {
    if report.warning_count() == 0 {
        return;
    }

    CliService::output(&format!(
        "  {} warning(s):",
        BrandColors::starting(&report.warning_count().to_string())
    ));

    for domain in &report.domains {
        for warning in &domain.warnings {
            CliService::output("");
            CliService::output(&format!(
                "  {} [{}] {}",
                BrandColors::starting("⚠"),
                domain.domain,
                warning.field
            ));
            CliService::output(&format!("    {}", warning.message));
            if let Some(ref suggestion) = warning.suggestion {
                CliService::output(&format!(
                    "    {} {}",
                    BrandColors::highlight("fix:"),
                    suggestion
                ));
            }
        }
    }

    CliService::output("");
}
