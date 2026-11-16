//! `admin agents show` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;

use crate::shared::CommandOutput;
use super::types::AgentDetailOutput;
use systemprompt_extension::cli::CommandHost;
use systemprompt_extension::cli::resolve_required;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: ShowArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let config = host.output();
    let prompter = host.prompter();
    let services_config = crate::commands::admin::state::boot(host)
        .await?
        .load_services_config()
        .context("Failed to load services configuration")?;

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
