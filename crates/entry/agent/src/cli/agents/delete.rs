//! `admin agents delete` command: removes agent definitions from the services
//! tree.
//!
//! An agent the service table records as running is refused with
//! [`AgentDeleteError::Running`], which names the kernel command that stops
//! it; this command never stops or signals a process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;

use super::types::AgentDeleteOutput;
use super::{reload_services_config, services_config, services_root};
use systemprompt_agent::services::agent_orchestration::status_view::AgentStatusView;
use systemprompt_agent::services::config_authoring::{
    AgentConfigAuthoringService, ConfigAuthoringError,
};
use systemprompt_extension::cli::{
    CliConfig, CommandHost, CommandOutput, Prompter, require_confirmation, resolve_required,
};
use systemprompt_identifiers::AgentName;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(
        help = "Agent name (required in non-interactive mode)",
        value_parser = systemprompt_extension::cli::parsers::parse_agent_name
    )]
    pub name: Option<AgentName>,

    #[arg(long, help = "Delete all agents")]
    pub all: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

/// Why one agent definition was not deleted.
#[derive(Debug, thiserror::Error)]
pub enum AgentDeleteError {
    #[error(
        "agent '{agent}' is running; stop it first with `systemprompt infra services stop agent \
         {agent}`"
    )]
    Running { agent: AgentName },
    #[error("failed to delete agent '{agent}'")]
    Remove {
        agent: AgentName,
        #[source]
        source: Box<ConfigAuthoringError>,
    },
}

pub(super) async fn execute(args: DeleteArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let prompter = host.prompter();
    let config = host.output();
    let services_config = services_config(host).await?;

    let agents_to_delete = resolve_targets(&args, prompter, &services_config, config)?;

    require_confirmation(
        prompter,
        &delete_confirm_message(args.all, &agents_to_delete),
        args.yes,
        config,
    )?;

    let authoring = AgentConfigAuthoringService::new(services_root(host)?);
    let running = running_agents(host).await?;

    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    for agent_name in &agents_to_delete {
        match delete_single_agent(agent_name, running.contains(agent_name), &authoring) {
            Ok(()) => deleted.push(agent_name.clone()),
            Err(error) => errors.push(format!("{:#}", anyhow::Error::new(error))),
        }
    }

    if !deleted.is_empty() {
        reload_services_config(host).with_context(|| {
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

async fn running_agents(host: &dyn CommandHost) -> Result<Vec<AgentName>> {
    let statuses = host
        .resolved()
        .await?
        .get::<Arc<AgentStatusView>>()?
        .statuses()
        .await
        .context("Failed to read the agent service table")?;
    Ok(statuses
        .into_iter()
        .filter(|status| status.pid.is_some())
        .map(|status| AgentName::new(status.name.as_str()))
        .collect())
}

fn resolve_targets(
    args: &DeleteArgs,
    prompter: &dyn Prompter,
    services_config: &systemprompt_manifest::ServicesConfig,
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

pub fn delete_single_agent(
    agent_name: &AgentName,
    running: bool,
    authoring: &AgentConfigAuthoringService,
) -> Result<(), AgentDeleteError> {
    if running {
        return Err(AgentDeleteError::Running {
            agent: agent_name.clone(),
        });
    }
    authoring
        .delete(agent_name)
        .map_err(|source| AgentDeleteError::Remove {
            agent: agent_name.clone(),
            source: Box::new(source),
        })
}
