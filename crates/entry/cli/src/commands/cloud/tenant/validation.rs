use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;
use systemprompt_loader::{ConfigLoader, ExtensionLoader};
use systemprompt_logging::CliService;
use systemprompt_models::ServicesConfig;

use crate::shared::project::ProjectRoot;

pub struct BuildValidationResult {
    pub required_secrets: Vec<String>,
}

pub fn check_build_ready() -> Result<(), String> {
    validate_build_ready()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn validate_build_ready() -> Result<BuildValidationResult> {
    let project_root =
        ProjectRoot::discover().context("Must be in a SystemPrompt project directory")?;
    let root = project_root.as_path();

    let binary_path = root.join("target/release/systemprompt");
    if !binary_path.exists() {
        bail!(
            "Release binary not found: {}\n\nRun: cargo build --release --bin systemprompt",
            binary_path.display()
        );
    }

    let extension_result = ExtensionLoader::validate(root);
    if !extension_result.missing_binaries.is_empty() {
        let missing_list = extension_result.format_missing_binaries();
        bail!(
            "MCP extension binaries not found:\n\n{}\n\nRun: just build --release",
            missing_list
        );
    }

    let services_path = find_services_config(root)?;
    let services_config = ConfigLoader::load_from_path(&services_path).with_context(|| {
        format!(
            "Failed to load services config: {}",
            services_path.display()
        )
    })?;

    let required_secrets = validate_ai_config(&services_config)?;

    Ok(BuildValidationResult { required_secrets })
}

pub fn find_services_config(root: &Path) -> Result<std::path::PathBuf> {
    let path = root.join("services/config/config.yaml");
    if path.exists() {
        return Ok(path);
    }
    bail!("Services config not found.\n\nExpected at: services/config/config.yaml");
}

fn validate_ai_config(services_config: &ServicesConfig) -> Result<Vec<String>> {
    let ai = &services_config.ai;
    let mut required_secrets = vec![];

    if ai.default_provider.is_empty() {
        bail!(
            "AI config missing default_provider.\n\nSet default_provider in \
             services/ai/config.yaml (e.g., default_provider: \"anthropic\")"
        );
    }

    let provider = ai.providers.get(&ai.default_provider).ok_or_else(|| {
        anyhow!(
            "Default provider '{}' not found in providers.\n\nAdd '{}' to ai.providers in your \
             config.",
            ai.default_provider,
            ai.default_provider
        )
    })?;

    if !provider.enabled {
        bail!(
            "Default provider '{}' is disabled.\n\nSet enabled: true for the '{}' provider.",
            ai.default_provider,
            ai.default_provider
        );
    }

    for (name, prov) in &ai.providers {
        if prov.enabled {
            let secret_key = match name.as_str() {
                "anthropic" => "ANTHROPIC_API_KEY",
                "openai" => "OPENAI_API_KEY",
                "google" => "GOOGLE_API_KEY",
                _ => continue,
            };
            required_secrets.push(secret_key.to_string());
        }
    }

    Ok(required_secrets)
}

pub fn warn_required_secrets(required_secrets: &[String]) {
    if required_secrets.is_empty() {
        return;
    }

    CliService::warning("Deployment requires API keys to be set via secrets:");
    for secret in required_secrets {
        CliService::info(&format!("  • {}", secret));
    }
    CliService::info("");
    CliService::info("Set secrets with: systemprompt cloud secrets set <KEY> <VALUE>");
    CliService::warning("Your deployment won't work until these secrets are configured.");
}
