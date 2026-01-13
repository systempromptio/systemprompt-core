use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use super::types::AgentDeleteOutput;
use systemprompt_core_logging::CliService;
use systemprompt_loader::ConfigLoader;

#[derive(Args)]
pub struct DeleteArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(long, help = "Delete all agents")]
    pub all: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<AgentDeleteOutput>> {
    let services_config = ConfigLoader::load()
        .context("Failed to load services configuration")?;

    let agents_to_delete: Vec<String> = if args.all {
        services_config.agents.keys().cloned().collect()
    } else {
        let name = resolve_input(
            args.name,
            "name",
            config,
            || prompt_agent_selection(&services_config),
        )?;

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

    CliService::warning(
        "Agent deletion modifies configuration files. \
         Please remove the agent configuration from your services.yaml manually for now."
    );

    let output = AgentDeleteOutput {
        deleted: agents_to_delete.clone(),
        message: format!(
            "Agent(s) prepared for deletion. Remove the following from services.yaml:\n\n{}",
            agents_to_delete
                .iter()
                .map(|n| format!("agents:\n  {}: # Remove this entire block", n))
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
    };

    Ok(CommandResult::text(output).with_title("Delete Agent"))
}

fn prompt_agent_selection(
    config: &systemprompt_models::ServicesConfig,
) -> Result<String> {
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
