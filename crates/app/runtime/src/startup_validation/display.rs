use systemprompt_logging::services::cli::BrandColors;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

pub fn display_validation_report(report: &StartupValidationReport) {
    println!();
    println!(
        "{} {}",
        BrandColors::stopped("✗"),
        BrandColors::white_bold("Validation Failed")
    );

    if let Some(ref path) = report.profile_path {
        println!(
            "  {} {}",
            BrandColors::dim("Profile:"),
            BrandColors::highlight(&path.display().to_string())
        );
    }

    println!();
    println!(
        "  {} error(s) found:",
        BrandColors::stopped(&report.error_count().to_string())
    );

    for domain in &report.domains {
        display_domain_errors(domain);
    }

    for ext in &report.extensions {
        display_extension_errors(ext);
    }

    println!();
}

fn display_domain_errors(domain: &ValidationReport) {
    if !domain.has_errors() {
        return;
    }

    println!();
    println!(
        "  {} {}",
        BrandColors::stopped("▸"),
        BrandColors::white_bold(&domain.domain)
    );

    for error in &domain.errors {
        println!("    {} {}", BrandColors::dim("field:"), error.field);
        println!("    {} {}", BrandColors::dim("error:"), error.message);
        if let Some(ref path) = error.path {
            println!("    {} {}", BrandColors::dim("path:"), path.display());
        }
        if let Some(ref suggestion) = error.suggestion {
            println!("    {} {}", BrandColors::highlight("fix:"), suggestion);
        }
    }
}

fn display_extension_errors(ext: &ValidationReport) {
    if !ext.has_errors() {
        return;
    }

    println!();
    println!(
        "  {} {}",
        BrandColors::stopped("▸"),
        BrandColors::white_bold(&ext.domain)
    );

    for error in &ext.errors {
        println!("    {} {}", BrandColors::dim("field:"), error.field);
        println!("    {} {}", BrandColors::dim("error:"), error.message);
    }
}

pub fn display_validation_warnings(report: &StartupValidationReport) {
    if report.warning_count() == 0 {
        return;
    }

    println!(
        "  {} warning(s):",
        BrandColors::starting(&report.warning_count().to_string())
    );

    for domain in &report.domains {
        for warning in &domain.warnings {
            println!();
            println!(
                "  {} [{}] {}",
                BrandColors::starting("⚠"),
                domain.domain,
                warning.field
            );
            println!("    {}", warning.message);
            if let Some(ref suggestion) = warning.suggestion {
                println!("    {} {}", BrandColors::highlight("fix:"), suggestion);
            }
        }
    }

    println!();
}
