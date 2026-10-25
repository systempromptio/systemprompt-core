//! `admin agents edit` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;

use super::edit_apply::{
    apply_capability_fields, apply_card_fields, apply_enabled_flags, apply_mcp_server_changes,
    apply_metadata_fields, apply_runtime_fields, apply_set_value_changes, apply_skill_changes,
};
use super::shared::AgentArgs;
use super::types::AgentEditOutput;
use crate::context::CommandContext;
use crate::interactive::resolve_required;
use crate::shared::CommandOutput;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::ConfigWriter;
use systemprompt_logging::CliService;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(
        help = "Agent name (required in non-interactive mode)",
        value_parser = crate::shared::parse_agent_name
    )]
    pub name: Option<AgentName>,

    #[arg(
        long = "set",
        value_name = "KEY=VALUE",
        help = "Set a configuration value (advanced)"
    )]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the agent", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the agent", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long = "remove-mcp-server", help = "Remove an MCP server reference")]
    pub remove_mcp_servers: Vec<String>,

    #[arg(long = "remove-skill", help = "Remove a skill reference")]
    pub remove_skills: Vec<String>,

    #[command(flatten)]
    pub agent: AgentArgs,
}

pub(super) fn execute(args: &EditArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let config = &ctx.cli;
    let prompter = ctx.prompter();
    let services_config = ctx
        .boot()?
        .load_services_config()
        .context("Failed to load services configuration")?;

    let name = resolve_required(args.name.clone(), "name", config, || {
        super::shared::prompt_agent_selection(prompter, "Select agent to edit", &services_config)
            .map(AgentName::new)
    })?;

    let mut agent = services_config
        .agents
        .get(name.as_str())
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?
        .clone();

    let mut changes = Vec::new();
    apply_enabled_flags(&mut agent, args, &mut changes);
    apply_runtime_fields(&mut agent, args, &mut changes)?;
    apply_card_fields(&mut agent, args, &mut changes);
    apply_capability_fields(&mut agent, args, &mut changes);
    apply_metadata_fields(&mut agent, args, &mut changes)?;
    apply_mcp_server_changes(&mut agent, args, &services_config, &mut changes)?;
    apply_skill_changes(&mut agent, args, &mut changes);
    apply_set_value_changes(&mut agent, args, &mut changes)?;

    if changes.is_empty() {
        return Err(anyhow!(
            "No changes specified. Use flags like --port, --display-name, --provider, --model, \
             --mcp-server, --skill, --system-prompt, --enable/--disable, etc."
        ));
    }

    CliService::info(&format!("Updating agent '{}'...", name));

    let profile = ctx.profile()?;
    let services_dir = Path::new(&profile.paths.services);

    ConfigWriter::update_agent(&name, &agent, services_dir)
        .with_context(|| format!("Failed to update agent '{}'", name))?;

    ctx.boot()?.load_services_config().with_context(|| {
        format!(
            "Agent '{}' updated but validation failed. Please check the configuration.",
            name
        )
    })?;

    CliService::success(&format!("Agent '{}' updated successfully", name));

    let output = AgentEditOutput {
        name: name.clone(),
        message: format!(
            "Agent '{}' updated successfully with {} change(s)",
            name,
            changes.len()
        ),
        changes,
    };

    Ok(CommandOutput::card_value(
        format!("Edit Agent: {}", name),
        &output,
    ))
}
