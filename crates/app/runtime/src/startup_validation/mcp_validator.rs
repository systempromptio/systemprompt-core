//! MCP deployment manifest validation folded into the startup report.
//!
//! An internal server is spawned one of two ways, decided by its manifest:
//! with `binary:` it runs that extension binary, which links and checks its
//! own toolsets; with no `binary:` and a `toolsets:` list it is hosted by this
//! binary (`admin mcp serve`), so every listed toolset id must be registered
//! in this binary through [`RegisteredToolsetId`]. A binary that links no
//! toolsets has an empty inventory, so every hosted toolset in it is reported:
//! an empty inventory is never read as "nothing to check".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use systemprompt_loader::ExtensionBinaryIndex;
use systemprompt_manifest::validators::validation_report::ValidationIssue;
use systemprompt_manifest::validators::{StartupValidationReport, ValidationReport};
use systemprompt_manifest::{Config, ServicesConfig};
use systemprompt_models::mcp::McpServerType;
use systemprompt_models::mcp::deployment::Deployment;

/// One toolset id linked into this binary, submitted alongside its factory.
#[derive(Debug, Clone, Copy)]
pub struct RegisteredToolsetId {
    pub id: &'static str,
}

systemprompt_mcp::__inventory::collect!(RegisteredToolsetId);

#[must_use]
pub fn registered_toolset_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = systemprompt_mcp::__inventory::iter::<RegisteredToolsetId>
        .into_iter()
        .map(|registered| registered.id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub(super) fn validate_mcp_manifests(
    config: &Config,
    services_config: &ServicesConfig,
    report: &mut StartupValidationReport,
) {
    let registry = ExtensionBinaryIndex::build(
        Path::new(&config.system_path),
        config.is_cloud,
        &config.bin_path,
    );

    let known = registered_toolset_ids();
    let mcp_errors = collect_manifest_errors(services_config, config.is_cloud, &known, |binary| {
        registry.get_path(binary).map(|_| ())
    });

    merge_mcp_errors(report, mcp_errors);
}

pub fn collect_toolset_errors(
    services_config: &ServicesConfig,
    known: &[&str],
) -> Vec<ValidationIssue> {
    let mut names: Vec<&String> = services_config.mcp_servers.keys().collect();
    names.sort();
    names
        .into_iter()
        .filter(|name| services_config.mcp_servers[*name].enabled)
        .flat_map(|name| hosted_toolset_errors(name, &services_config.mcp_servers[name], known))
        .collect()
}

fn hosted_toolset_errors(
    name: &str,
    deployment: &Deployment,
    known: &[&str],
) -> Vec<ValidationIssue> {
    let registered = if known.is_empty() {
        "none (this binary links no toolsets)".to_owned()
    } else {
        known.join(", ")
    };
    deployment
        .enabled_toolsets()
        .filter(|toolset| !known.contains(&toolset.id.as_str()))
        .map(|toolset| {
            ValidationIssue::new(
                format!("mcp_servers.{name}.toolsets"),
                format!(
                    "Toolset '{}' is not registered by any crate linked into this binary",
                    toolset.id
                ),
            )
            .with_suggestion(format!(
                "Registered toolsets: {registered}. Link the crate that registers it with \
                 submit_toolset!, or fix the id"
            ))
        })
        .collect()
}

pub fn collect_manifest_errors<F, E>(
    services_config: &ServicesConfig,
    is_cloud: bool,
    known_toolsets: &[&str],
    resolve: F,
) -> Vec<ValidationIssue>
where
    F: Fn(&str) -> Result<(), E>,
    E: std::fmt::Display,
{
    let mut mcp_errors: Vec<ValidationIssue> = Vec::new();

    for (name, deployment) in &services_config.mcp_servers {
        if !deployment.enabled {
            continue;
        }
        if deployment.dev_only && is_cloud {
            continue;
        }
        if deployment.tool_policy.is_none() {
            mcp_errors.push(
                ValidationIssue::new(
                    format!("mcp_servers.{}.tool_policy", name),
                    "No tool_policy declared; the server is withheld from the bridge manifest"
                        .to_owned(),
                )
                .with_suggestion(format!(
                    "Set tool_policy: allow | deny | prompt in services/mcp/{}.yaml",
                    name
                )),
            );
        }
        if !matches!(deployment.server_type, McpServerType::Internal) {
            continue;
        }

        let Some(binary) = deployment.binary.as_deref() else {
            if deployment.enabled_toolsets().next().is_none() {
                mcp_errors.push(
                    ValidationIssue::new(
                        format!("mcp_servers.{}.binary", name),
                        "Internal server declares neither a binary nor toolsets".to_owned(),
                    )
                    .with_suggestion(format!(
                        "Set binary: <extension binary>, or list toolsets: hosted by this \
                         binary, in services/mcp/{}.yaml",
                        name
                    )),
                );
            } else {
                mcp_errors.extend(hosted_toolset_errors(name, deployment, known_toolsets));
            }
            continue;
        };

        if let Err(e) = resolve(binary) {
            mcp_errors.push(
                ValidationIssue::new(
                    format!("mcp_servers.{}.binary", name),
                    format!("Manifest not found for binary '{binary}': {e}"),
                )
                .with_suggestion(format!(
                    "Ensure manifest.yaml exists at extensions/mcp/{binary}/manifest.yaml"
                )),
            );
        }
    }

    mcp_errors
}

pub fn merge_mcp_errors(report: &mut StartupValidationReport, mcp_errors: Vec<ValidationIssue>) {
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
