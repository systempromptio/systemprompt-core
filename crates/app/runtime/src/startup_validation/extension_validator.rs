use std::path::Path;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::services::cli::{render_phase_success, BrandColors};
use systemprompt_models::Config;
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

use super::config_loaders::load_extension_config;

#[allow(clippy::print_stdout)]
pub fn validate_extensions(config: &Config, report: &mut StartupValidationReport, verbose: bool) {
    let extensions = ExtensionRegistry::discover();
    let config_extensions = extensions.config_extensions();
    let asset_extensions = extensions.asset_extensions();

    let has_extensions = !config_extensions.is_empty() || !asset_extensions.is_empty();

    if !has_extensions {
        return;
    }

    if verbose {
        println!();
        println!(
            "{} {}",
            BrandColors::primary("▸"),
            BrandColors::white_bold("Validating extensions")
        );
    }

    for ext in config_extensions {
        validate_single_extension(config, ext.as_ref(), report, verbose);
    }

    validate_extension_assets(&extensions, report, verbose);
}

#[allow(clippy::print_stdout)]
fn validate_extension_assets(
    registry: &ExtensionRegistry,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    for ext in registry.asset_extensions() {
        let ext_id = ext.id();
        let mut has_errors = false;

        for asset in ext.required_assets() {
            if asset.is_required() && !asset.source().exists() {
                has_errors = true;
                let mut ext_report = ValidationReport::new(format!("ext:{}", ext_id));
                ext_report.add_error(
                    ValidationError::new(
                        "required_asset",
                        format!("Missing required asset: {}", asset.source().display()),
                    )
                    .with_suggestion("Ensure the asset file exists at the specified path"),
                );
                report.add_extension(ext_report);

                println!(
                    "  {} [ext:{}] Missing asset: {}",
                    BrandColors::stopped("✗"),
                    ext_id,
                    asset.source().display()
                );
            }
        }

        if !has_errors && verbose {
            render_phase_success(&format!("[ext:{}]", ext_id), Some("assets valid"));
        }
    }
}

#[allow(clippy::print_stdout)]
fn validate_single_extension(
    config: &Config,
    ext: &dyn systemprompt_extension::Extension,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    let ext_id = ext.id();
    let Some(prefix) = ext.config_prefix() else {
        return;
    };

    let config_path = Path::new(&config.services_path)
        .join("config")
        .join(format!("{}.yaml", prefix));

    let config_json = if config_path.exists() {
        match load_extension_config(&config_path) {
            Ok(json) => json,
            Err(e) => {
                let mut ext_report = ValidationReport::new(format!("ext:{}", ext_id));
                ext_report.add_error(ValidationError::new(
                    format!("{}.config", prefix),
                    format!("Failed to load config: {}", e),
                ));
                report.add_extension(ext_report);
                println!("  {} [ext:{}] {}", BrandColors::stopped("✗"), ext_id, e);
                return;
            },
        }
    } else {
        serde_json::json!({})
    };

    match ext.validate_config(&config_json) {
        Ok(()) => {
            if verbose {
                render_phase_success(&format!("[ext:{}]", ext_id), Some("valid"));
            }
        },
        Err(e) => {
            let mut ext_report = ValidationReport::new(format!("ext:{}", ext_id));
            ext_report.add_error(ValidationError::new(
                format!("{}.config", prefix),
                e.to_string(),
            ));
            report.add_extension(ext_report);
            println!("  {} [ext:{}] {}", BrandColors::stopped("✗"), ext_id, e);
        },
    }
}
