use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use std::sync::Arc;

use super::types::AgentDeleteOutput;
use crate::CliConfig;
use crate::interactive::{Prompter, require_confirmation, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::config_authoring::AgentConfigAuthoringService;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;
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

pub(super) async fn execute(
    args: DeleteArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let agents_to_delete = resolve_targets(&args, prompter, &services_config, config)?;

    let confirm_message = if args.all {
        format!("Delete ALL {} agents?", agents_to_delete.len())
    } else {
        format!("Delete agent '{}'?", agents_to_delete[0])
    };

    require_confirmation(prompter, &confirm_message, args.yes, config)?;

    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let authoring = AgentConfigAuthoringService::new(Path::new(&profile.paths.services));

    let orchestrator = match build_orchestrator().await {
        Ok(orchestrator) => orchestrator,
        Err(message) => {
            return Ok(CommandOutput::card_value(
                "Delete Failed",
                &AgentDeleteOutput {
                    deleted: vec![],
                    message,
                },
            ));
        },
    };

    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for agent_name in &agents_to_delete {
        let agent_port = services_config.agents.get(agent_name).map(|c| c.port);
        let result = delete_single_agent(
            agent_name,
            agent_port,
            orchestrator.as_ref(),
            &authoring,
            args.force,
        )
        .await;
        match result {
            Ok(()) => deleted.push(agent_name.clone()),
            Err(msg) => errors.push(msg),
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

    Ok(CommandOutput::card_value("Delete Agent", &output))
}

fn resolve_targets(
    args: &DeleteArgs,
    prompter: &dyn Prompter,
    services_config: &systemprompt_models::ServicesConfig,
    config: &CliConfig,
) -> Result<Vec<String>> {
    let agents: Vec<String> = if args.all {
        services_config.agents.keys().cloned().collect()
    } else {
        let name = resolve_required(args.name.clone(), "name", config, || {
            super::shared::prompt_agent_selection(
                prompter,
                "Select agent to delete",
                services_config,
            )
        })?;

        if !services_config.agents.contains_key(&name) {
            return Err(anyhow!("Agent '{}' not found", name));
        }

        vec![name]
    };

    if agents.is_empty() {
        return Err(anyhow!("No agents to delete"));
    }

    Ok(agents)
}

async fn build_orchestrator() -> Result<Option<AgentOrchestrator>, String> {
    let ctx = match AppContext::new().await {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::debug!(error = %e, "Failed to create AppContext for agent deletion");
            return Ok(None);
        },
    };

    let jwt_provider = match JwtValidationProviderImpl::from_config() {
        Ok(p) => Arc::new(p),
        Err(e) => {
            tracing::debug!(error = %e, "Failed to create JWT provider");
            return Err(format!("Failed to initialize: {e}"));
        },
    };

    let agent_state = Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    ));

    Ok(
        AgentOrchestrator::new(agent_state, Arc::clone(ctx.app_paths_arc()), None)
            .await
            .ok(),
    )
}

async fn delete_single_agent(
    agent_name: &str,
    agent_port: Option<u16>,
    orchestrator: Option<&AgentOrchestrator>,
    authoring: &AgentConfigAuthoringService,
    force: bool,
) -> Result<(), String> {
    CliService::info(&format!("Deleting agent '{}'...", agent_name));

    let process_stopped = stop_agent_process(agent_name, agent_port, orchestrator).await;

    if !process_stopped && !force {
        let msg = format!(
            "Failed to stop agent '{}' process. Use --force to delete anyway.",
            agent_name
        );
        CliService::error(&msg);
        return Err(msg);
    }

    if !process_stopped && force {
        CliService::warning(&format!(
            "Force deleting agent '{}' (process may still be running)",
            agent_name
        ));
    }

    match authoring.delete(agent_name) {
        Ok(()) => {
            CliService::success(&format!("Agent '{}' deleted", agent_name));
            Ok(())
        },
        Err(e) => {
            CliService::error(&format!("Failed to delete agent '{}': {}", agent_name, e));
            Err(format!("{}: {}", agent_name, e))
        },
    }
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

    let Some(pid) = ProcessCleanup::check_port(port) else {
        tracing::warn!(agent = %agent_name, port, "No process found on port to stop");
        return false;
    };

    if !ProcessCleanup::kill_process(pid) {
        tracing::warn!(agent = %agent_name, port, pid, "Failed to kill process on port");
        return false;
    }

    tracing::debug!(agent = %agent_name, port, pid, "Killed process on port");
    true
}
