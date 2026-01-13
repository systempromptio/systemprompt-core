use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use super::types::AgentDetailOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_loader::ConfigLoader;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,
}

pub async fn execute(
    args: ShowArgs,
    config: &CliConfig,
) -> Result<CommandResult<AgentDetailOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let name = resolve_input(args.name, "name", config, || {
        prompt_agent_selection(&services_config)
    })?;

    let agent = services_config
        .agents
        .get(&name)
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

    let provider = agent
        .metadata
        .provider
        .clone()
        .unwrap_or_else(|| "-".to_string());
    let model = agent
        .metadata
        .model
        .clone()
        .unwrap_or_else(|| "-".to_string());

    let output = AgentDetailOutput {
        name: agent.name.clone(),
        display_name: agent.card.display_name.clone(),
        description: agent.card.description.clone(),
        port: agent.port,
        endpoint: agent.endpoint.clone(),
        enabled: agent.enabled,
        provider,
        model,
        mcp_servers: agent.metadata.mcp_servers.clone(),
        skills_count: agent.card.skills.len(),
    };

    Ok(CommandResult::card(output).with_title(format!("Agent: {}", name)))
}

fn prompt_agent_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut agents: Vec<&String> = config.agents.keys().collect();
    agents.sort();

    if agents.is_empty() {
        return Err(anyhow!("No agents configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent")
        .items(&agents)
        .default(0)
        .interact()
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
