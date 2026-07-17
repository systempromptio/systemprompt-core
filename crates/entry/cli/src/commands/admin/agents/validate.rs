//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;

use super::types::{ValidationIssue, ValidationOutput};
use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::profile::ProviderRegistry;
use systemprompt_models::secrets::Secrets;
use systemprompt_models::{AgentConfig, ServicesConfig};

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "Agent name to validate (optional)")]
    pub name: Option<String>,
}

struct ValidationSources<'a> {
    services_config: &'a ServicesConfig,
    registry: &'a ProviderRegistry,
    secrets: Option<&'a Secrets>,
}

pub(super) fn execute(args: &ValidateArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let registry = &ProfileBootstrap::get()
        .context("Failed to access bootstrapped profile for provider registry")?
        .providers;
    let secrets = SecretsBootstrap::get().ok();
    let sources = ValidationSources {
        services_config: &services_config,
        registry,
        secrets,
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut agents_checked = 0;

    let agents_to_check: Vec<(&String, &AgentConfig)> = match &args.name {
        Some(name) => {
            let agent = services_config
                .agents
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name))?;
            vec![(name, agent)]
        },
        None => services_config.agents.iter().collect(),
    };

    for (name, agent) in agents_to_check {
        agents_checked += 1;
        check_basics(name, agent, &mut errors, &mut warnings);
        check_provider(name, agent, &sources, &mut errors);
        check_mcp_references(name, agent, &services_config, &mut errors);
    }

    let output = ValidationOutput {
        valid: errors.is_empty(),
        items_checked: agents_checked,
        errors,
        warnings,
    };

    Ok(CommandOutput::card_value("Validation Results", &output))
}

fn check_basics(
    name: &str,
    agent: &AgentConfig,
    errors: &mut Vec<ValidationIssue>,
    warnings: &mut Vec<ValidationIssue>,
) {
    if let Err(e) = agent.validate(name) {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: e.to_string(),
            suggestion: None,
        });
    }

    if agent.port == 0 {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: "Port cannot be 0".to_owned(),
            suggestion: None,
        });
    }

    if agent.card.display_name.is_empty() {
        warnings.push(ValidationIssue {
            source: name.to_owned(),
            message: "Display name is empty".to_owned(),
            suggestion: None,
        });
    }

    if agent.card.description.is_empty() {
        warnings.push(ValidationIssue {
            source: name.to_owned(),
            message: "Description is empty".to_owned(),
            suggestion: None,
        });
    }

    if agent.enabled && agent.metadata.provider.is_none() {
        warnings.push(ValidationIssue {
            source: name.to_owned(),
            message: "Enabled agent has no AI provider configured".to_owned(),
            suggestion: None,
        });
    }
}

fn check_provider(
    name: &str,
    agent: &AgentConfig,
    sources: &ValidationSources<'_>,
    errors: &mut Vec<ValidationIssue>,
) {
    if !agent.enabled {
        return;
    }
    let Some(provider_name) = &agent.metadata.provider else {
        return;
    };

    let Some(provider_config) = sources.services_config.ai.providers.get(provider_name) else {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: format!(
                "Provider '{}' is not configured in ai.providers",
                provider_name
            ),
            suggestion: None,
        });
        return;
    };

    if !provider_config.enabled {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: format!(
                "Provider '{}' is disabled in AI config (set enabled: true)",
                provider_name
            ),
            suggestion: None,
        });
    }

    match sources.registry.find_provider(provider_name) {
        None => {
            errors.push(ValidationIssue {
                source: name.to_owned(),
                message: format!(
                    "Provider '{}' has no connectivity entry in the profile registry",
                    provider_name
                ),
                suggestion: None,
            });
        },
        Some(entry) => {
            let secret_name = entry.api_key_secret.as_str();
            let key_present = sources
                .secrets
                .and_then(|s| s.get(secret_name))
                .is_some_and(|k| !k.is_empty());
            if !key_present {
                errors.push(ValidationIssue {
                    source: name.to_owned(),
                    message: format!(
                        "No API key configured for provider '{}' (secret '{}' not found)",
                        provider_name, secret_name
                    ),
                    suggestion: None,
                });
            }
        },
    }
}

fn check_mcp_references(
    name: &str,
    agent: &AgentConfig,
    services_config: &ServicesConfig,
    errors: &mut Vec<ValidationIssue>,
) {
    for mcp_server in &agent.metadata.mcp_servers.include {
        if !services_config.mcp_servers.contains_key(mcp_server) {
            errors.push(ValidationIssue {
                source: name.to_owned(),
                message: format!("Referenced MCP server '{}' not found in config", mcp_server),
                suggestion: None,
            });
        }
    }
}
