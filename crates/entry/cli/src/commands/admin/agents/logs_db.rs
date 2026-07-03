use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use systemprompt_database::DbPool;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::LoggingRepository;
use systemprompt_runtime::AppContext;

use super::logs::LogsArgs;
use super::types::AgentLogsOutput;
use crate::CliConfig;
use crate::shared::CommandOutput;

pub(super) async fn execute_db_mode(args: &LogsArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize app context")?,
    );
    execute_db_mode_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_db_mode_with_pool(
    args: &LogsArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = LoggingRepository::new(pool)?;

    let patterns = match &args.agent {
        Some(agent) => build_agent_patterns(agent),
        None => build_all_agent_patterns()?,
    };

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
    vec![
        format!("%{}%", agent),
        "%agent%".to_owned(),
        "%a2a%".to_owned(),
    ]
}

fn build_all_agent_patterns() -> Result<Vec<String>> {
    let services_config = ConfigLoader::load().context("Failed to load services config")?;

    let mut patterns: Vec<String> = services_config
        .agents
        .keys()
        .flat_map(|name| vec![format!("%{}%", name)])
        .collect();

    patterns.push("%agent%".to_owned());
    patterns.push("%a2a%".to_owned());
    patterns.push("%orchestration%".to_owned());

    Ok(patterns)
}
