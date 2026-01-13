use anyhow::{Context, Result};
use clap::Args;

use crate::shared::CommandResult;
use crate::CliConfig;
use super::types::{AgentListOutput, AgentSummary};
use systemprompt_loader::ConfigLoader;

#[derive(Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled agents")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled agents", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub async fn execute(
    args: ListArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AgentListOutput>> {
    let services_config = ConfigLoader::load()
        .context("Failed to load services configuration")?;

    let mut agents: Vec<AgentSummary> = services_config
        .agents
        .iter()
        .filter(|(_, agent)| {
            if args.enabled {
                agent.enabled
            } else if args.disabled {
                !agent.enabled
            } else {
                true
            }
        })
        .map(|(name, agent)| AgentSummary {
            name: name.clone(),
            display_name: agent.card.display_name.clone(),
            port: agent.port,
            enabled: agent.enabled,
            is_primary: agent.is_primary,
            is_default: agent.default,
        })
        .collect();

    agents.sort_by(|a, b| a.name.cmp(&b.name));

    let output = AgentListOutput { agents };

    Ok(CommandResult::table(output)
        .with_title("Agents")
        .with_columns(vec![
            "name".to_string(),
            "display_name".to_string(),
            "port".to_string(),
            "enabled".to_string(),
            "is_primary".to_string(),
            "is_default".to_string(),
        ]))
}
