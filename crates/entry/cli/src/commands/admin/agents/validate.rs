use anyhow::{Context, Result};
use clap::Args;

use super::types::{ValidationIssue, ValidationOutput};
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::SecretsBootstrap;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "Agent name to validate (optional)")]
    pub name: Option<String>,
}

pub fn execute(
    args: &ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ValidationOutput>> {
    let mut services_config =
        ConfigLoader::load().context("Failed to load services configuration")?;

    if let Ok(secrets) = SecretsBootstrap::get() {
        for provider_config in services_config.ai.providers.values_mut() {
            if provider_config.api_key.starts_with("${") && provider_config.api_key.ends_with('}') {
                let var_name =
                    provider_config.api_key[2..provider_config.api_key.len() - 1].to_string();
                if let Some(v) = secrets.get(&var_name) {
                    provider_config.api_key.clone_from(v);
                }
            }
        }
    }

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
                message: "Port cannot be 0".to_string(),
                suggestion: None,
            });
        }

        if agent.card.display_name.is_empty() {
            warnings.push(ValidationIssue {
                source: name.clone(),
                message: "Display name is empty".to_string(),
                suggestion: None,
            });
        }

        if agent.card.description.is_empty() {
            warnings.push(ValidationIssue {
                source: name.clone(),
                message: "Description is empty".to_string(),
                suggestion: None,
            });
        }

        if agent.enabled && agent.metadata.provider.is_none() {
            warnings.push(ValidationIssue {
                source: name.clone(),
                message: "Enabled agent has no AI provider configured".to_string(),
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

                        if provider_config.api_key.is_empty()
                            || provider_config.api_key.starts_with("${")
                        {
                            errors.push(ValidationIssue {
                                source: name.clone(),
                                message: format!(
                                    "No API key configured for provider '{}' (check secrets file)",
                                    provider_name
                                ),
                                suggestion: None,
                            });
                        }
                    },
                }
            }
        }

        for mcp_server in &agent.metadata.mcp_servers {
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

    Ok(CommandResult::table(output).with_title("Validation Results"))
}
