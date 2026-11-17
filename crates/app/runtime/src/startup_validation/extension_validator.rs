//! Per-member config and asset validation at startup, over the admitted
//! composition: admission has already dropped every member the profile
//! disables, so each admitted member's declared config sections and required
//! assets are checked.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_config::paths::AppPaths;
use systemprompt_extension::contribute::{ConfigDecl, ContributionSink};
use systemprompt_extension::{AdmittedConfigs, BuiltState, ExtensionConfigError};
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::BootContext;
use systemprompt_logging::CliService;
use systemprompt_logging::services::cli::{BrandColors, render_phase_success};
use systemprompt_manifest::Config;
use systemprompt_manifest::validators::validation_report::ValidationIssue;
use systemprompt_manifest::validators::{StartupValidationReport, ValidationReport};

use super::config_loaders::{ConfigFileError, load_extension_config};

pub(super) fn validate_extensions(
    built: &BuiltState,
    boot: &BootContext,
    config: &Config,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    let has_extensions = built
        .members()
        .any(|member| !member.sink.configs().is_empty() || !member.sink.assets().is_empty());

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

    for member in built.members() {
        for decl in member.sink.configs() {
            validate_single_config(config, &member.owner, decl, report, verbose);
        }
    }

    let profile = boot.profile();
    match AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        boot.services_root().map(|r| r.path.as_path()),
    ) {
        Ok(paths) => {
            for member in built.members() {
                validate_member_assets(&member.owner, &member.sink, &paths, report, verbose);
            }
        },
        Err(e) => {
            let mut ext_report = ValidationReport::new("ext:assets".to_owned());
            ext_report.add_error(ValidationIssue::new(
                "extension_asset_paths",
                format!("Failed to resolve the profile paths for asset validation: {e}"),
            ));
            report.add_extension(ext_report);
        },
    }
}

fn validate_member_assets(
    ext_id: &ExtensionId,
    sink: &ContributionSink,
    paths: &AppPaths,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    if sink.assets().is_empty() {
        return;
    }
    let mut has_errors = false;

    for asset in sink.assets().iter().flat_map(|assets| assets(paths)) {
        if asset.is_required() && !asset.source().exists() {
            has_errors = true;
            let mut ext_report = ValidationReport::new(format!("ext:{ext_id}"));
            ext_report.add_error(
                ValidationIssue::new(
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
        render_phase_success(&format!("[ext:{ext_id}]"), Some("assets valid"));
    }
}

/// A single extension's resolved config-validation result.
///
/// Produced by [`validate_extension_configs`] so callers outside the serve
/// boot path (e.g. the `cloud doctor` preflight) can run the exact same
/// config validation pass without a `Config` or a [`StartupValidationReport`].
#[derive(Debug)]
pub struct ExtensionConfigOutcome {
    pub extension_id: ExtensionId,
    pub config_key: String,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
enum ExtConfigError {
    #[error("{0}")]
    Load(#[source] ConfigFileError),
    #[error("{0}")]
    Validate(#[source] ExtensionConfigError),
}

fn evaluate_config(decl: &ConfigDecl, services_path: &Path) -> Result<(), ExtConfigError> {
    let config_path = services_path
        .join("config")
        .join(format!("{}.yaml", decl.prefix()));

    let config_json = if config_path.exists() {
        load_extension_config(&config_path).map_err(ExtConfigError::Load)?
    } else {
        serde_json::json!({})
    };

    decl.validate(&config_json)
        .map_err(ExtConfigError::Validate)
}

#[must_use]
pub fn validate_extension_configs(
    configs: &AdmittedConfigs,
    services_path: &Path,
) -> Vec<ExtensionConfigOutcome> {
    configs
        .decls()
        .iter()
        .map(|(owner, decl)| ExtensionConfigOutcome {
            extension_id: owner.clone(),
            config_key: format!("{}.config", decl.prefix()),
            error: evaluate_config(decl, services_path)
                .err()
                .map(|e| e.to_string()),
        })
        .collect()
}

fn validate_single_config(
    config: &Config,
    ext_id: &ExtensionId,
    decl: &ConfigDecl,
    report: &mut StartupValidationReport,
    verbose: bool,
) {
    let prefix = decl.prefix();
    match evaluate_config(decl, Path::new(&config.services_path)) {
        Ok(()) => {
            if verbose {
                render_phase_success(&format!("[ext:{ext_id}]"), Some("valid"));
            }
        },
        Err(e) => {
            let report_message = match &e {
                ExtConfigError::Load(cause) => format!("Failed to load config: {cause}"),
                ExtConfigError::Validate(cause) => cause.to_string(),
            };
            let mut ext_report = ValidationReport::new(format!("ext:{ext_id}"));
            ext_report.add_error(ValidationIssue::new(
                format!("{prefix}.config"),
                report_message,
            ));
            report.add_extension(ext_report);
            CliService::output(&format!(
                "  {} [ext:{}] {}",
                BrandColors::stopped("✗"),
                ext_id,
                e
            ));
        },
    }
}
