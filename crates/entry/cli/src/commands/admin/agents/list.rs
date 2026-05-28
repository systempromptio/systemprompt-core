use anyhow::{Context, Result, anyhow};
use clap::Args;

use super::types::{AgentDetailOutput, AgentListOutput, AgentSummary};
use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_loader::ConfigLoader;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Agent name to show details (optional)")]
    pub name: Option<String>,

    #[arg(long, help = "Show only enabled agents")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled agents", conflicts_with = "enabled")]
    pub disabled: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub(super) enum ListOrDetail {
    List(AgentListOutput),
    Detail(AgentDetailOutput),
}

pub(super) fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ListOrDetail>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    if let Some(name) = args.name {
        let agent = services_config
            .agents
            .get(&name)
            .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

        let output = AgentDetailOutput {
            name: agent.name.clone(),
            display_name: agent.card.display_name.clone(),
            description: agent.card.description.clone(),
            port: agent.port,
            endpoint: agent.endpoint.clone(),
            enabled: agent.enabled,
            provider: agent
                .metadata
                .provider
                .clone()
                .unwrap_or_else(|| "-".to_owned()),
            model: agent
                .metadata
                .model
                .clone()
                .unwrap_or_else(|| "-".to_owned()),
            mcp_servers: agent.metadata.mcp_servers.include.clone(),
            skills_count: agent.metadata.skills.include.len(),
        };

        return Ok(CommandResult::card(ListOrDetail::Detail(output))
            .with_title(format!("Agent: {}", name)));
    }

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
        .map(|(name, agent)| AgentSummary::from_config(name, agent))
        .collect();

    agents.sort_by(|a, b| a.name.cmp(&b.name));

    let output = AgentListOutput { agents };

    Ok(CommandResult::table(ListOrDetail::List(output))
        .with_title("Agents")
        .with_columns(vec![
            "name".to_owned(),
            "display_name".to_owned(),
            "port".to_owned(),
            "enabled".to_owned(),
            "is_primary".to_owned(),
            "is_default".to_owned(),
        ]))
}
