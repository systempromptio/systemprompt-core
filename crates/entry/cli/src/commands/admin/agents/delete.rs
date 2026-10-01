//! `admin agents delete` command with target resolution and orchestrated
//! teardown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use std::sync::Arc;

use super::types::AgentDeleteOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::interactive::{Prompter, require_confirmation, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use systemprompt_agent::services::agent_orchestration::{AgentOrchestrator, AgentStatus};
use systemprompt_agent::services::config_authoring::AgentConfigAuthoringService;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;
use systemprompt_oauth::JwtValidationProviderImpl;
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

pub(super) async fn execute(args: DeleteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let prompter = ctx.prompter();
    let config = &ctx.cli;
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let agents_to_delete = resolve_targets(&args, prompter, &services_config, config)?;

    require_confirmation(
        prompter,
        &delete_confirm_message(args.all, &agents_to_delete),
        args.yes,
        config,
    )?;

    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let authoring = AgentConfigAuthoringService::new(Path::new(&profile.paths.services));

    let orchestrator = build_orchestrator(ctx)
        .await
        .context("Cannot stop agents safely without the orchestrator")?;

    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for agent_name in &agents_to_delete {
        let agent_port = services_config.agents.get(agent_name).map(|c| c.port);
        let process_stopped = stop_agent_process(agent_name, agent_port, &orchestrator).await;
        match delete_single_agent(agent_name, process_stopped, &authoring, args.force) {
            Ok(()) => deleted.push(agent_name.clone()),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }

    if !deleted.is_empty() {
        ConfigLoader::reload().with_context(|| {
            "Agent(s) deleted but configuration validation failed. Please check the configuration."
        })?;
    }

    if !errors.is_empty() {
        return Err(anyhow!(
            "{}\nFailed to delete:\n{}",
            delete_success_message(&deleted),
            errors.join("\n")
        ));
    }

    let message = delete_success_message(&deleted);
    let output = AgentDeleteOutput { deleted, message };

    Ok(CommandOutput::card_value("Delete Agent", &output))
}

fn resolve_targets(
    args: &DeleteArgs,
    prompter: &dyn Prompter,
    services_config: &systemprompt_models::ServicesConfig,
    config: &CliConfig,
) -> Result<Vec<String>> {
    let available: Vec<String> = services_config.agents.keys().cloned().collect();
    let requested = if args.all {
        None
    } else {
        Some(resolve_required(args.name.clone(), "name", config, || {
            super::shared::prompt_agent_selection(
                prompter,
                "Select agent to delete",
                services_config,
            )
        })?)
    };
    validate_delete_targets(requested, &available)
}

pub fn validate_delete_targets(
    requested: Option<String>,
    available: &[String],
) -> Result<Vec<String>> {
    let agents = match requested {
        Some(name) => {
            if !available.contains(&name) {
                return Err(anyhow!("Agent '{}' not found", name));
            }
            vec![name]
        },
        None => available.to_vec(),
    };

    if agents.is_empty() {
        return Err(anyhow!("No agents to delete"));
    }

    Ok(agents)
}

#[must_use]
pub fn delete_confirm_message(all: bool, targets: &[String]) -> String {
    if all {
        format!("Delete ALL {} agents?", targets.len())
    } else {
        format!("Delete agent '{}'?", targets[0])
    }
}

#[must_use]
pub fn delete_success_message(deleted: &[String]) -> String {
    if deleted.len() == 1 {
        format!("Agent '{}' deleted successfully", deleted[0])
    } else {
        format!("{} agent(s) deleted successfully", deleted.len())
    }
}

async fn build_orchestrator(ctx: &CommandContext) -> Result<AgentOrchestrator> {
    let app = ctx.app_context().await?;
    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );
    let broadcaster = HttpWebhookBroadcaster::from_config(app.config())
        .context("Failed to initialize webhook broadcaster")?;

    let agent_state = Arc::new(AgentState::new(
        Arc::clone(app.db_pool()),
        Arc::new(app.config().clone()),
        jwt_provider,
        Arc::clone(app.a2a_repositories()),
        Arc::new(broadcaster),
    ));

    AgentOrchestrator::new(agent_state, Arc::clone(app.app_paths_arc()), None)
        .await
        .context("Failed to initialize agent orchestrator")
}

pub fn delete_single_agent(
    agent_name: &str,
    process_stopped: bool,
    authoring: &AgentConfigAuthoringService,
    force: bool,
) -> Result<()> {
    CliService::info(&format!("Deleting agent '{}'...", agent_name));

    if !process_stopped && !force {
        let msg = format!(
            "Failed to stop agent '{}' process. Use --force to delete anyway.",
            agent_name
        );
        CliService::error(&msg);
        return Err(anyhow!(msg));
    }

    if !process_stopped {
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
            Err(e).with_context(|| format!("Failed to delete agent '{agent_name}'"))
        },
    }
}

pub async fn stop_agent_process(
    agent_name: &str,
    agent_port: Option<u16>,
    orchestrator: &AgentOrchestrator,
) -> bool {
    let typed_name = systemprompt_identifiers::AgentName::new(agent_name);
    let recorded_pid = match orchestrator.get_status(&typed_name).await {
        Ok(AgentStatus::Running { pid, .. }) => Some(pid),
        Ok(AgentStatus::Failed { .. }) => None,
        Err(e) => {
            tracing::warn!(
                agent = %agent_name,
                error = %e,
                "Could not read the agent's recorded process"
            );
            return false;
        },
    };

    match orchestrator.delete_agent(&typed_name).await {
        Ok(()) => {
            tracing::debug!(agent = %agent_name, "Agent stopped via orchestrator");
            true
        },
        Err(e) => {
            tracing::warn!(
                agent = %agent_name,
                error = %e,
                "Orchestrator termination failed; stopping only a verified agent process"
            );
            stop_verified_port_holder(agent_name, agent_port, recorded_pid)
        },
    }
}

pub fn stop_verified_port_holder(
    agent_name: &str,
    agent_port: Option<u16>,
    recorded_pid: Option<u32>,
) -> bool {
    let Some(port) = agent_port else {
        return recorded_pid.is_none();
    };

    let Some(holder) = ProcessCleanup::check_port(port) else {
        tracing::debug!(agent = %agent_name, port, "No process on port; agent is stopped");
        return true;
    };

    if recorded_pid != Some(holder) {
        CliService::warning(&format!(
            "Process {holder} holds port {port} but is not the recorded process of agent \
             '{agent_name}'; refusing to kill it"
        ));
        return false;
    }

    CliService::info(&format!(
        "Stopping agent '{}' (pid {}) on port {}...",
        agent_name, holder, port
    ));
    if !ProcessCleanup::kill_process(holder) {
        tracing::warn!(agent = %agent_name, port, pid = holder, "Failed to kill agent process");
        return false;
    }
    true
}
