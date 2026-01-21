use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::path::Path;
use std::sync::Arc;

use super::types::AgentDeleteOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::AgentState;
use systemprompt_loader::{ConfigLoader, ConfigWriter};
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::ProcessCleanup;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(long, help = "Delete all agents")]
    pub all: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,

    #[arg(long, help = "Force delete even if process cannot be stopped")]
    pub force: bool,
}

pub async fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<AgentDeleteOutput>> {
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

    let orchestrator = match AppContext::new().await {
        Ok(ctx) => {
            let jwt_provider = match JwtValidationProviderImpl::from_config() {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    tracing::debug!(error = %e, "Failed to create JWT provider");
                    return Ok(CommandResult::text(AgentDeleteOutput {
                        deleted: vec![],
                        message: format!("Failed to initialize: {e}"),
                    })
                    .with_title("Delete Failed"));
                },
            };
            let agent_state = Arc::new(AgentState::new(
                Arc::clone(ctx.db_pool()),
                Arc::new(ctx.config().clone()),
                jwt_provider,
            ));
            AgentOrchestrator::new(agent_state, None).await.ok()
        },
        Err(e) => {
            tracing::debug!(error = %e, "Failed to create AppContext for agent deletion");
            None
        },
    };

    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for agent_name in &agents_to_delete {
        CliService::info(&format!("Deleting agent '{}'...", agent_name));

        let agent_port = services_config.agents.get(agent_name).map(|c| c.port);

        let process_stopped =
            stop_agent_process(agent_name, agent_port, orchestrator.as_ref()).await;

        if !process_stopped && !args.force {
            let msg = format!(
                "Failed to stop agent '{}' process. Use --force to delete anyway.",
                agent_name
            );
            CliService::error(&msg);
            errors.push(msg);
            continue;
        }

        if !process_stopped && args.force {
            CliService::warning(&format!(
                "Force deleting agent '{}' (process may still be running)",
                agent_name
            ));
        }

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

async fn stop_agent_process(
    agent_name: &str,
    agent_port: Option<u16>,
    orchestrator: Option<&AgentOrchestrator>,
) -> bool {
    if let Some(orch) = orchestrator {
        match orch.delete_agent(agent_name).await {
            Ok(()) => {
                tracing::debug!(agent = %agent_name, "Agent stopped via orchestrator");
                return true;
            },
            Err(e) => {
                tracing::debug!(
                    agent = %agent_name,
                    error = %e,
                    "Orchestrator termination failed, trying port-based cleanup"
                );
            },
        }
    }

    let Some(port) = agent_port else {
        tracing::debug!(agent = %agent_name, "No port configured, assuming not running");
        return true;
    };

    if ProcessCleanup::check_port(port).is_none() {
        tracing::debug!(agent = %agent_name, port, "No process on port, assuming stopped");
        return true;
    }

    CliService::info(&format!(
        "Stopping agent '{}' on port {}...",
        agent_name, port
    ));

    let killed = ProcessCleanup::kill_port(port);
    if killed.is_empty() {
        tracing::warn!(agent = %agent_name, port, "Failed to kill process on port");
        return false;
    }

    tracing::debug!(agent = %agent_name, port, pids = ?killed, "Killed processes on port");
    true
}
