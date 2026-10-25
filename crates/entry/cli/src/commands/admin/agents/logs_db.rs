//! Database-log patterns for `admin agents logs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use anyhow::{Context, Result, anyhow};
use systemprompt_database::DbPool;
use systemprompt_logging::LoggingRepository;
use systemprompt_manifest::ServicesConfig;

use super::logs::LogsArgs;
use super::types::AgentLogsOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

pub(super) async fn execute_db_mode(
    args: &LogsArgs,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let app = ctx
        .app_context()
        .await
        .context("Failed to initialize app context")?;
    let services = ctx
        .boot()?
        .load_services_config()
        .context("Failed to load services config")?;
    execute_db_mode_with_pool(args, app.db_pool(), &services, &ctx.cli).await
}

pub async fn execute_db_mode_with_pool(
    args: &LogsArgs,
    pool: &DbPool,
    services: &ServicesConfig,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = LoggingRepository::new(pool);

    let patterns = args.agent.as_ref().map_or_else(
        || build_all_agent_patterns(services),
        |agent| build_agent_patterns(agent),
    );

    let entries = repo
        .get_logs_by_module_patterns(&patterns, args.lines as i64)
        .await
        .context("Failed to query logs from database")?;

    if entries.is_empty() {
        return Err(anyhow!("No logs found in database for agents"));
    }

    let logs: Vec<String> = entries
        .iter()
        .map(|e| {
            let message = console::strip_ansi_codes(&e.message);
            format!(
                "{} {} [{}] {}",
                e.timestamp.format("%Y-%m-%d %H:%M:%S"),
                e.level,
                e.module,
                message
            )
        })
        .filter(|line| !line.contains("[profile:"))
        .collect();

    let agent_label = args.agent.clone().unwrap_or_else(|| "all".to_owned());

    Ok(CommandOutput::card_value(
        format!("Agent Logs (DB): {}", agent_label),
        &AgentLogsOutput {
            agent: Some(agent_label),
            source: "database".to_owned(),
            logs,
            log_files: vec![],
        },
    ))
}

fn build_agent_patterns(agent: &str) -> Vec<String> {
    vec![format!("%{}%", agent)]
}

fn build_all_agent_patterns(services: &ServicesConfig) -> Vec<String> {
    let mut patterns: Vec<String> = services
        .agents
        .keys()
        .flat_map(|name| vec![format!("%{}%", name)])
        .collect();

    patterns.push("%agent%".to_owned());
    patterns.push("%a2a%".to_owned());
    patterns.push("%orchestration%".to_owned());

    patterns
}
