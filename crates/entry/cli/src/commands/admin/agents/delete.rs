//! `admin agents delete` command with target resolution and orchestrated
//! teardown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use std::sync::Arc;

use super::process_stop::stop_agent_process;
use super::types::AgentDeleteOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::interactive::{Prompter, require_confirmation, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::config_authoring::AgentConfigAuthoringService;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;
use systemprompt_oauth::JwtValidationProviderImpl;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(
        help = "Agent name (required in non-interactive mode)",
        value_parser = crate::shared::parse_agent_name
    )]
    pub name: Option<AgentName>,

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
        let agent_port = services_config
            .agents
            .get(agent_name.as_str())
            .map(|c| c.port);
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
) -> Result<Vec<AgentName>> {
    let available: Vec<AgentName> = services_config.agents.keys().map(AgentName::new).collect();
    let requested = if args.all {
        None
    } else {
        Some(resolve_required(args.name.clone(), "name", config, || {
            super::shared::prompt_agent_selection(
                prompter,
                "Select agent to delete",
                services_config,
            )
            .map(AgentName::new)
        })?)
    };
    validate_delete_targets(requested, &available)
}

pub fn validate_delete_targets(
    requested: Option<AgentName>,
    available: &[AgentName],
) -> Result<Vec<AgentName>> {
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
pub fn delete_confirm_message(all: bool, targets: &[AgentName]) -> String {
    if all {
        format!("Delete ALL {} agents?", targets.len())
    } else {
        format!("Delete agent '{}'?", targets[0])
    }
}

#[must_use]
pub fn delete_success_message(deleted: &[AgentName]) -> String {
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
    agent_name: &AgentName,
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
