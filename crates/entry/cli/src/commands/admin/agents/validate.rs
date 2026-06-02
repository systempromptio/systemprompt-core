use anyhow::{Context, Result};
use clap::Args;

use super::types::{ValidationIssue, ValidationOutput};
use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_loader::ConfigLoader;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "Agent name to validate (optional)")]
    pub name: Option<String>,
}

pub(super) fn execute(args: &ValidateArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let registry = &ProfileBootstrap::get()
        .context("Failed to access bootstrapped profile for provider registry")?
        .providers;
    let secrets = SecretsBootstrap::get().ok();

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut agents_checked = 0;

    let agents_to_check: Vec<(&String, &systemprompt_models::AgentConfig)> = match &args.name {
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

        if let Err(e) = agent.validate(name) {
            errors.push(ValidationIssue {
                source: name.clone(),
                message: e.to_string(),
                suggestion: None,
            });
        }

        if agent.port == 0 {
            errors.push(ValidationIssue {
                source: name.clone(),
                message: "Port cannot be 0".to_owned(),
                suggestion: None,
            });
        }

        if agent.card.display_name.is_empty() {
            warnings.push(ValidationIssue {
                source: name.clone(),
                message: "Display name is empty".to_owned(),
                suggestion: None,
            });
        }

        if agent.card.description.is_empty() {
            warnings.push(ValidationIssue {
                source: name.clone(),
                message: "Description is empty".to_owned(),
                suggestion: None,
            });
        }

        if agent.enabled && agent.metadata.provider.is_none() {
            warnings.push(ValidationIssue {
                source: name.clone(),
                message: "Enabled agent has no AI provider configured".to_owned(),
                suggestion: None,
            });
        }

        if agent.enabled {
            if let Some(provider_name) = &agent.metadata.provider {
                match services_config.ai.providers.get(provider_name) {
                    None => {
                        errors.push(ValidationIssue {
                            source: name.clone(),
                            message: format!(
                                "Provider '{}' is not configured in ai.providers",
                                provider_name
                            ),
                            suggestion: None,
                        });
                    },
                    Some(provider_config) => {
                        if !provider_config.enabled {
                            errors.push(ValidationIssue {
                                source: name.clone(),
                                message: format!(
                                    "Provider '{}' is disabled in AI config (set enabled: true)",
                                    provider_name
                                ),
                                suggestion: None,
                            });
                        }

                        match registry.find_provider(provider_name) {
                            None => {
                                errors.push(ValidationIssue {
                                    source: name.clone(),
                                    message: format!(
                                        "Provider '{}' has no connectivity entry in the profile \
                                         registry",
                                        provider_name
                                    ),
                                    suggestion: None,
                                });
                            },
                            Some(entry) => {
                                let secret_name = entry.api_key_secret.as_str();
                                let key_present = secrets
                                    .as_ref()
                                    .and_then(|s| s.get(secret_name))
                                    .is_some_and(|k| !k.is_empty());
                                if !key_present {
                                    errors.push(ValidationIssue {
                                        source: name.clone(),
                                        message: format!(
                                            "No API key configured for provider '{}' (secret '{}' \
                                             not found)",
                                            provider_name, secret_name
                                        ),
                                        suggestion: None,
                                    });
                                }
                            },
                        }
                    },
                }
            }
        }

        for mcp_server in &agent.metadata.mcp_servers.include {
            if !services_config.mcp_servers.contains_key(mcp_server) {
                errors.push(ValidationIssue {
                    source: name.clone(),
                    message: format!("Referenced MCP server '{}' not found in config", mcp_server),
                    suggestion: None,
                });
            }
        }
    }

    let output = ValidationOutput {
        valid: errors.is_empty(),
        items_checked: agents_checked,
        errors,
        warnings,
    };

    Ok(CommandOutput::card_value("Validation Results", &output))
}
