//! MCP deployment manifest validation folded into the startup report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use systemprompt_loader::ExtensionRegistry as McpExtensionRegistry;
use systemprompt_models::mcp::McpServerType;
use systemprompt_models::{Config, ServicesConfig};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

pub(super) fn validate_mcp_manifests(
    config: &Config,
    services_config: &ServicesConfig,
    report: &mut StartupValidationReport,
) {
    let registry = McpExtensionRegistry::build(
        Path::new(&config.system_path),
        config.is_cloud,
        &config.bin_path,
    );

    let mcp_errors = collect_manifest_errors(services_config, config.is_cloud, |binary| {
        registry
            .get_path(binary)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });

    merge_mcp_errors(report, mcp_errors);
}

pub fn collect_manifest_errors<F>(
    services_config: &ServicesConfig,
    is_cloud: bool,
    resolve: F,
) -> Vec<ValidationError>
where
    F: Fn(&str) -> Result<(), String>,
{
    let mut mcp_errors: Vec<ValidationError> = Vec::new();

    for (name, deployment) in &services_config.mcp_servers {
        if !deployment.enabled {
            continue;
        }
        if deployment.dev_only && is_cloud {
            continue;
        }
        if !matches!(deployment.server_type, McpServerType::Internal) {
            continue;
        }

        if let Err(e) = resolve(&deployment.binary) {
            mcp_errors.push(
                ValidationError::new(
                    format!("mcp_servers.{}.binary", name),
                    format!(
                        "Manifest not found for binary '{}': {}",
                        deployment.binary, e
                    ),
                )
                .with_suggestion(format!(
                    "Ensure manifest.yaml exists at extensions/mcp/{}/manifest.yaml",
                    deployment.binary
                )),
            );
        }
    }

    mcp_errors
}

pub fn merge_mcp_errors(report: &mut StartupValidationReport, mcp_errors: Vec<ValidationError>) {
    if mcp_errors.is_empty() {
        return;
    }

    if let Some(mcp_report) = report.domains.iter_mut().find(|d| d.domain == "mcp") {
        for error in mcp_errors {
            mcp_report.add_error(error);
        }
    } else {
        let mut mcp_report = ValidationReport::new("mcp");
        for error in mcp_errors {
            mcp_report.add_error(error);
        }
        report.add_domain(mcp_report);
    }
}
