use anyhow::{Result, bail};
use std::collections::HashSet;
use std::path::Path;

use systemprompt_cloud::constants::container;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::ServicesConfig;

pub fn get_required_mcp_copy_lines(
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Vec<String> {
    ExtensionLoader::get_production_mcp_binary_names(project_root, services_config)
        .iter()
        .map(|bin| format!("COPY target/release/{} {}/", bin, container::BIN))
        .collect()
}

fn extract_mcp_binary_names_from_dockerfile(dockerfile_content: &str) -> Vec<String> {
    dockerfile_content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with("COPY target/release/systemprompt-") {
                return None;
            }
            let after_copy = trimmed.strip_prefix("COPY target/release/")?;
            let binary_name = after_copy.split_whitespace().next()?;
            if binary_name.starts_with("systemprompt-") && binary_name != "systemprompt-*" {
                Some(binary_name.to_string())
            } else {
                None
            }
        })
        .collect()
}

pub fn validate_dockerfile_has_mcp_binaries(
    dockerfile_content: &str,
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Vec<String> {
    let has_wildcard = dockerfile_content.contains("target/release/systemprompt-*");
    if has_wildcard {
        return Vec::new();
    }

    ExtensionLoader::get_production_mcp_binary_names(project_root, services_config)
        .into_iter()
        .filter(|binary| {
            let expected_pattern = format!("target/release/{}", binary);
            !dockerfile_content.contains(&expected_pattern)
        })
        .collect()
}

pub fn validate_dockerfile_has_no_stale_binaries(
    dockerfile_content: &str,
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Vec<String> {
    let has_wildcard = dockerfile_content.contains("target/release/systemprompt-*");
    if has_wildcard {
        return Vec::new();
    }

    let dockerfile_binaries = extract_mcp_binary_names_from_dockerfile(dockerfile_content);
    let current_binaries: HashSet<String> =
        ExtensionLoader::get_production_mcp_binary_names(project_root, services_config)
            .into_iter()
            .collect();

    dockerfile_binaries
        .into_iter()
        .filter(|binary| !current_binaries.contains(binary))
        .collect()
}

pub fn validate_profile_dockerfile(
    dockerfile_path: &Path,
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Result<()> {
    if !dockerfile_path.exists() {
        bail!(
            "Dockerfile not found at {}\n\nCreate a profile first with: systemprompt cloud \
             profile create",
            dockerfile_path.display()
        );
    }

    let content = std::fs::read_to_string(dockerfile_path)?;
    let missing = validate_dockerfile_has_mcp_binaries(&content, project_root, services_config);
    let stale = validate_dockerfile_has_no_stale_binaries(&content, project_root, services_config);

    match (missing.is_empty(), stale.is_empty()) {
        (true, true) => Ok(()),
        (false, true) => {
            bail!(
                "Dockerfile at {} is missing COPY commands for MCP binaries:\n\n{}\n\nAdd these \
                 lines:\n\n{}",
                dockerfile_path.display(),
                missing.join(", "),
                get_required_mcp_copy_lines(project_root, services_config).join("\n")
            );
        },
        (true, false) => {
            bail!(
                "Dockerfile at {} has COPY commands for dev-only or removed \
                 binaries:\n\n{}\n\nRemove these lines or regenerate with: systemprompt cloud \
                 profile create",
                dockerfile_path.display(),
                stale.join(", ")
            );
        },
        (false, false) => {
            bail!(
                "Dockerfile at {} has issues:\n\nMissing binaries: {}\nDev-only/stale binaries: \
                 {}\n\nRegenerate with: systemprompt cloud profile create",
                dockerfile_path.display(),
                missing.join(", "),
                stale.join(", ")
            );
        },
    }
}
