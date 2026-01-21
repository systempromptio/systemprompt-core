use std::path::Path;
use systemprompt_loader::ExtensionRegistry as McpExtensionRegistry;
use systemprompt_models::{Config, ServicesConfig};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{StartupValidationReport, ValidationReport};

pub fn validate_mcp_manifests(
    config: &Config,
    services_config: &ServicesConfig,
    report: &mut StartupValidationReport,
) {
    let registry = McpExtensionRegistry::build(
        Path::new(&config.system_path),
        config.is_cloud,
        &config.bin_path,
    );

    let mut mcp_errors: Vec<ValidationError> = Vec::new();

    for (name, deployment) in &services_config.mcp_servers {
        if !deployment.enabled {
            continue;
        }
        if deployment.dev_only && config.is_cloud {
            continue;
        }

        if let Err(e) = registry.get_path(&deployment.binary) {
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
