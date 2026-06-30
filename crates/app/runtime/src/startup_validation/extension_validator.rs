use std::path::Path;
use systemprompt_config::ProfileBootstrap;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_logging::services::cli::{BrandColors, render_phase_success};
use systemprompt_models::{AppPaths, Config};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

use super::config_loaders::load_extension_config;

pub(super) fn validate_extensions(
    config: &Config,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    let extensions = match ExtensionRegistry::discover() {
        Ok(extensions) => extensions,
        Err(e) => {
            let mut ext_report = ValidationReport::new("ext:registry".to_owned());
            ext_report.add_error(ValidationError::new(
                "extension_discovery",
                format!("Failed to discover extensions: {}", e),
            ));
            report.add_extension(ext_report);
            return;
        },
    };
    let config_extensions = extensions.config_extensions();
    let asset_extensions = extensions.asset_extensions();

    let has_extensions = !config_extensions.is_empty() || !asset_extensions.is_empty();

    if !has_extensions {
        return;
    }

    if verbose {
        CliService::output("");
        CliService::output(&format!(
            "{} {}",
            BrandColors::primary("▸"),
            BrandColors::white_bold("Validating extensions")
        ));
    }

    for ext in config_extensions {
        validate_single_extension(config, ext.as_ref(), report, verbose);
    }

    let paths_result = ProfileBootstrap::get()
        .map_err(|e| e.to_string())
        .and_then(|p| AppPaths::from_profile(&p.paths).map_err(|e| e.to_string()));

    match paths_result {
        Ok(paths) => validate_extension_assets(&extensions, &paths, report, verbose),
        Err(_) if verbose => {
            CliService::output(&format!(
                "  {} Asset validation skipped (profile not loaded)",
                BrandColors::dim("○")
            ));
        },
        Err(_) => {},
    }
}

fn validate_extension_assets(
    registry: &ExtensionRegistry,
    paths: &AppPaths,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    for ext in registry.asset_extensions() {
        let ext_id = ext.id();
        let mut has_errors = false;

        for asset in ext.required_assets(paths) {
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

                CliService::output(&format!(
                    "  {} [ext:{}] Missing asset: {}",
                    BrandColors::stopped("✗"),
                    ext_id,
                    asset.source().display()
                ));
            }
        }

        if !has_errors && verbose {
            render_phase_success(&format!("[ext:{}]", ext_id), Some("assets valid"));
        }
    }
}

/// A single extension's resolved config-validation result.
///
/// Produced by [`validate_extension_configs`] so callers outside the serve
/// boot path (e.g. the `cloud doctor` preflight) can run the exact same
/// `validate_config` pass without a `Config` or a [`StartupValidationReport`].
#[derive(Debug)]
pub struct ExtensionConfigOutcome {
    pub extension_id: String,
    pub config_key: String,
    pub error: Option<String>,
}

enum ExtConfigError {
    Load(String),
    Validate(String),
}

impl ExtConfigError {
    fn message(&self) -> &str {
        match self {
            Self::Load(m) | Self::Validate(m) => m,
        }
    }
}

fn evaluate_extension_config(
    ext: &dyn systemprompt_extension::Extension,
    services_path: &Path,
) -> Result<(), ExtConfigError> {
    let Some(prefix) = ext.config_prefix() else {
        return Ok(());
    };

    let config_path = services_path
        .join("config")
        .join(format!("{}.yaml", prefix));

    let config_json = if config_path.exists() {
        load_extension_config(&config_path).map_err(ExtConfigError::Load)?
    } else {
        serde_json::json!({})
    };

    ext.validate_config(&config_json)
        .map_err(|e| ExtConfigError::Validate(e.to_string()))
}

/// Run every config-bearing extension's `validate_config` against the resolved
/// service config under `services_path` and collect the results.
///
/// This is the same per-extension load-and-validate the serve boot path runs;
/// both paths funnel through `evaluate_extension_config` so they cannot drift.
/// `Err` indicates the extension registry could not be discovered at all.
pub fn validate_extension_configs(
    services_path: &Path,
) -> Result<Vec<ExtensionConfigOutcome>, String> {
    let extensions = ExtensionRegistry::discover().map_err(|e| e.to_string())?;

    Ok(extensions
        .config_extensions()
        .iter()
        .filter_map(|ext| {
            let prefix = ext.config_prefix()?;
            let error = evaluate_extension_config(ext.as_ref(), services_path)
                .err()
                .map(|e| e.message().to_owned());
            Some(ExtensionConfigOutcome {
                extension_id: ext.id().to_owned(),
                config_key: format!("{}.config", prefix),
                error,
            })
        })
        .collect())
}

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

    match evaluate_extension_config(ext, Path::new(&config.services_path)) {
        Ok(()) => {
            if verbose {
                render_phase_success(&format!("[ext:{}]", ext_id), Some("valid"));
            }
        },
        Err(e) => {
            let report_message = match &e {
                ExtConfigError::Load(m) => format!("Failed to load config: {}", m),
                ExtConfigError::Validate(m) => m.clone(),
            };
            let mut ext_report = ValidationReport::new(format!("ext:{}", ext_id));
            ext_report.add_error(ValidationError::new(
                format!("{}.config", prefix),
                report_message,
            ));
            report.add_extension(ext_report);
            CliService::output(&format!(
                "  {} [ext:{}] {}",
                BrandColors::stopped("✗"),
                ext_id,
                e.message()
            ));
        },
    }
}
