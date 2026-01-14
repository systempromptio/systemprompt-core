use anyhow::{Context, Result};
use clap::Args;

use super::types::{ValidationIssue, ValidationOutput, ValidationSeverity};
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_loader::ConfigLoader;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "Agent name to validate (optional)")]
    pub name: Option<String>,
}

pub fn execute(
    args: &ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ValidationOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let mut issues = Vec::new();
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
            issues.push(ValidationIssue {
                agent: name.clone(),
                severity: ValidationSeverity::Error,
                message: e.to_string(),
            });
        }

        if agent.port == 0 {
            issues.push(ValidationIssue {
                agent: name.clone(),
                severity: ValidationSeverity::Error,
                message: "Port cannot be 0".to_string(),
            });
        }

        if agent.card.display_name.is_empty() {
            issues.push(ValidationIssue {
                agent: name.clone(),
                severity: ValidationSeverity::Warning,
                message: "Display name is empty".to_string(),
            });
        }

        if agent.card.description.is_empty() {
            issues.push(ValidationIssue {
                agent: name.clone(),
                severity: ValidationSeverity::Warning,
                message: "Description is empty".to_string(),
            });
        }

        if agent.enabled && agent.metadata.provider.is_none() {
            issues.push(ValidationIssue {
                agent: name.clone(),
                severity: ValidationSeverity::Warning,
                message: "Enabled agent has no AI provider configured".to_string(),
            });
        }

        // Validate that the configured provider is enabled and has an API key
        if agent.enabled {
            if let Some(provider_name) = &agent.metadata.provider {
                match services_config.ai.providers.get(provider_name) {
                    None => {
                        issues.push(ValidationIssue {
                            agent: name.clone(),
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "Provider '{}' is not configured in ai.providers",
                                provider_name
                            ),
                        });
                    },
                    Some(provider_config) => {
                        if !provider_config.enabled {
                            issues.push(ValidationIssue {
                                agent: name.clone(),
                                severity: ValidationSeverity::Error,
                                message: format!(
                                    "Provider '{}' is disabled in AI config (set enabled: true)",
                                    provider_name
                                ),
                            });
                        }

                        // Check if API key is configured (non-empty or placeholder)
                        if provider_config.api_key.is_empty()
                            || provider_config.api_key.starts_with("${")
                        {
                            issues.push(ValidationIssue {
                                agent: name.clone(),
                                severity: ValidationSeverity::Error,
                                message: format!(
                                    "No API key configured for provider '{}' (check secrets file)",
                                    provider_name
                                ),
                            });
                        }
                    },
                }
            }
        }

        for mcp_server in &agent.metadata.mcp_servers {
            if !services_config.mcp_servers.contains_key(mcp_server) {
                issues.push(ValidationIssue {
                    agent: name.clone(),
                    severity: ValidationSeverity::Error,
                    message: format!("Referenced MCP server '{}' not found in config", mcp_server),
                });
            }
        }
    }

    let valid = issues
        .iter()
        .all(|i| matches!(i.severity, ValidationSeverity::Warning));

    let output = ValidationOutput {
        valid,
        agents_checked,
        issues,
    };

    Ok(CommandResult::table(output).with_title("Validation Results"))
}
