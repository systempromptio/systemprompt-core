//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use super::types::{AgentStatusOutput, AgentStatusRow};
use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::agent_orchestration::{AgentOrchestrator, AgentStatus};
use systemprompt_loader::ConfigLoader;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(help = "Agent name to check (optional)")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: StatusArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let ctx = AppContext::new()
        .await
        .context("Failed to initialize application context")?;

    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );
    let agent_state = Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    ));

    let orchestrator = AgentOrchestrator::new(agent_state, Arc::clone(ctx.app_paths_arc()), None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let all_statuses = orchestrator.list_all().await?;

    let agents_to_check: Vec<(&String, &systemprompt_models::AgentConfig)> = match &args.name {
        Some(name) => {
            let agent = services_config
                .agents
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name))?;
            vec![(name, agent)]
        },
        None => services_config.agents.iter().collect(),
    };

    let mut agents: Vec<AgentStatusRow> = Vec::new();

    for (name, agent) in agents_to_check {
        let status = all_statuses.iter().find(|(n, _)| n == name);
        let (is_running, pid) = match status {
            Some((_, AgentStatus::Running { pid, .. })) => (true, Some(*pid)),
            Some((_, AgentStatus::Failed { .. })) | None => (false, None),
        };

        agents.push(AgentStatusRow {
            name: name.clone(),
            enabled: agent.enabled,
            is_running,
            pid,
            port: agent.port,
        });
    }

    agents.sort_by(|a, b| a.name.cmp(&b.name));

    let output = AgentStatusOutput { agents };

    Ok(CommandOutput::table_of(
        vec!["name", "enabled", "is_running", "pid", "port"],
        &output.agents,
    )
    .with_title("Agent Status"))
}
