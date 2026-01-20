use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use super::types::{AgentStatusOutput, AgentStatusRow};
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_core_agent::services::agent_orchestration::{AgentOrchestrator, AgentStatus};
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(help = "Agent name to check (optional)")]
    pub name: Option<String>,
}

pub async fn execute(
    args: StatusArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AgentStatusOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let orchestrator = AgentOrchestrator::new(Arc::clone(&ctx), None)
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

    Ok(CommandResult::table(output)
        .with_title("Agent Status")
        .with_columns(vec![
            "name".to_string(),
            "enabled".to_string(),
            "is_running".to_string(),
            "pid".to_string(),
            "port".to_string(),
        ]))
}
