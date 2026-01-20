use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::path::Path;

use super::types::AgentDeleteOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_loader::{ConfigLoader, ConfigWriter};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(long, help = "Delete all agents")]
    pub all: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub fn execute(args: DeleteArgs, config: &CliConfig) -> Result<CommandResult<AgentDeleteOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let agents_to_delete: Vec<String> = if args.all {
        services_config.agents.keys().cloned().collect()
    } else {
        let name = resolve_input(args.name, "name", config, || {
            prompt_agent_selection(&services_config)
        })?;

        if !services_config.agents.contains_key(&name) {
            return Err(anyhow!("Agent '{}' not found", name));
        }

        vec![name]
    };

    if agents_to_delete.is_empty() {
        return Err(anyhow!("No agents to delete"));
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow!(
                "--yes is required to delete agents in non-interactive mode"
            ));
        }

        let confirm_message = if args.all {
            format!("Delete ALL {} agents?", agents_to_delete.len())
        } else {
            format!("Delete agent '{}'?", agents_to_delete[0])
        };

        if !CliService::confirm(&confirm_message)? {
            CliService::info("Cancelled");
            return Ok(CommandResult::text(AgentDeleteOutput {
                deleted: vec![],
                message: "Operation cancelled".to_string(),
            })
            .with_title("Delete Cancelled"));
        }
    }

    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let services_dir = Path::new(&profile.paths.services);

    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for agent_name in &agents_to_delete {
        CliService::info(&format!("Deleting agent '{}'...", agent_name));

        match ConfigWriter::delete_agent(agent_name, services_dir) {
            Ok(()) => {
                CliService::success(&format!("Agent '{}' deleted", agent_name));
                deleted.push(agent_name.clone());
            },
            Err(e) => {
                CliService::error(&format!("Failed to delete agent '{}': {}", agent_name, e));
                errors.push(format!("{}: {}", agent_name, e));
            },
        }
    }

    if !errors.is_empty() && deleted.is_empty() {
        return Err(anyhow!("Failed to delete agents:\n{}", errors.join("\n")));
    }

    if !deleted.is_empty() {
        ConfigLoader::load().with_context(|| {
            "Agent(s) deleted but configuration validation failed. Please check the configuration."
        })?;
    }

    let message = if deleted.len() == 1 {
        format!("Agent '{}' deleted successfully", deleted[0])
    } else {
        format!("{} agent(s) deleted successfully", deleted.len())
    };

    let output = AgentDeleteOutput { deleted, message };

    Ok(CommandResult::text(output).with_title("Delete Agent"))
}

fn prompt_agent_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut agents: Vec<&String> = config.agents.keys().collect();
    agents.sort();

    if agents.is_empty() {
        return Err(anyhow!("No agents configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent to delete")
        .items(&agents)
        .default(0)
        .interact()
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
