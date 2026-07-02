use anyhow::{Context, Result, anyhow};
use clap::Args;

use super::types::AgentDetailOutput;
use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_loader::ConfigLoader;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,
}

pub(super) fn execute(
    args: ShowArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let name = resolve_required(args.name, "name", config, || {
        super::shared::prompt_agent_selection(prompter, "Select agent", &services_config)
    })?;

    let agent = services_config
        .agents
        .get(&name)
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

    let provider = agent
        .metadata
        .provider
        .clone()
        .unwrap_or_else(|| "-".to_owned());
    let model = agent
        .metadata
        .model
        .clone()
        .unwrap_or_else(|| "-".to_owned());

    let output = AgentDetailOutput {
        name: agent.name.clone(),
        display_name: agent.card.display_name.clone(),
        description: agent.card.description.clone(),
        port: agent.port,
        endpoint: agent.endpoint.clone(),
        enabled: agent.enabled,
        provider,
        model,
        mcp_servers: agent.metadata.mcp_servers.include.clone(),
        skills_count: agent.metadata.skills.include.len(),
    };

    Ok(CommandOutput::card_value(
        format!("Agent: {}", name),
        &output,
    ))
}
