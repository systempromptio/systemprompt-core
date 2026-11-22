//! `admin agents status` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::services::agent_orchestration::status_view::AgentStatusView;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

use super::services_config;
use super::types::{AgentStatusOutput, AgentStatusRow};

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(help = "Agent name to check (optional)")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: StatusArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let services_config = services_config(host).await?;

    let statuses = host
        .resolved()
        .await?
        .get::<Arc<AgentStatusView>>()?
        .statuses()
        .await
        .context("Failed to read the agent service table")?;

    let agents_to_check: Vec<(&String, &systemprompt_manifest::AgentConfig)> = match &args.name {
        Some(name) => {
            let agent = services_config
                .agents
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name))?;
            vec![(name, agent)]
        },
        None => services_config.agents.iter().collect(),
    };

    let mut agents: Vec<AgentStatusRow> = agents_to_check
        .into_iter()
        .map(|(name, agent)| {
            let pid = statuses
                .iter()
                .find(|status| status.name.as_str() == name.as_str())
                .and_then(|status| status.pid);
            AgentStatusRow {
                name: name.clone(),
                enabled: agent.enabled,
                is_running: pid.is_some(),
                pid,
                port: agent.port,
            }
        })
        .collect();

    agents.sort_by(|a, b| a.name.cmp(&b.name));

    let output = AgentStatusOutput { agents };

    Ok(CommandOutput::table_of(
        vec!["name", "enabled", "is_running", "pid", "port"],
        &output.agents,
    )
    .with_title("Agent Status"))
}
