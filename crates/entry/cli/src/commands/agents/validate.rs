use anyhow::{Context, Result};
use clap::Args;

use crate::shared::CommandResult;
use crate::CliConfig;
use super::types::{ValidationOutput, ValidationIssue, ValidationSeverity};
use systemprompt_loader::ConfigLoader;

#[derive(Args)]
pub struct ValidateArgs {
    #[arg(help = "Agent name to validate (optional)")]
    pub name: Option<String>,
}

pub async fn execute(
    args: ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ValidationOutput>> {
    let services_config = ConfigLoader::load()
        .context("Failed to load services configuration")?;

    let mut issues = Vec::new();
    let mut agents_checked = 0;

    let agents_to_check: Vec<(&String, &systemprompt_models::AgentConfig)> = match &args.name {
        Some(name) => {
            let agent = services_config
                .agents
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name))?;
            vec![(name, agent)]
        }
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

    let valid = issues.iter().all(|i| matches!(i.severity, ValidationSeverity::Warning));

    let output = ValidationOutput {
        valid,
        agents_checked,
        issues,
    };

    Ok(CommandResult::table(output).with_title("Validation Results"))
}
