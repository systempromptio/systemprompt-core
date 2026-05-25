use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::LoggingRepository;
use systemprompt_runtime::AppContext;

use super::logs::{LogLevel, LogsArgs};
use super::types::McpLogsOutput;
use crate::CliConfig;
use crate::shared::CommandResult;

pub(super) async fn execute_db_mode(
    args: &LogsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<McpLogsOutput>> {
    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize app context")?,
    );
    let repo = LoggingRepository::new(ctx.db_pool())?;

    let patterns = match &args.server {
        Some(service) => build_service_patterns(service),
        None => build_all_mcp_patterns()?,
    };

    let fetch_limit = if args.level.is_some() {
        (args.lines * 5) as i64
    } else {
        args.lines as i64
    };

    let entries = repo
        .get_logs_by_module_patterns(&patterns, fetch_limit)
        .await
        .context("Failed to query logs from database")?;

    if entries.is_empty() {
        return Err(anyhow!("No logs found in database for MCP services"));
    }

    let logs: Vec<String> = entries
        .iter()
        .filter(|e| {
            args.level
                .is_none_or(|level: LogLevel| level.matches(&e.level.to_string()))
        })
        .take(args.lines)
        .map(|e| {
            format!(
                "{} {} [{}] {}",
                e.timestamp.format("%Y-%m-%d %H:%M:%S"),
                e.level,
                e.module,
                e.message
            )
        })
        .collect();

    let service_label = args.server.clone().unwrap_or_else(|| "all".to_owned());
    let level_label = args.level.map_or_else(String::new, |l| {
        format!(" [{}+]", format!("{:?}", l).to_uppercase())
    });

    Ok(CommandResult::text(McpLogsOutput {
        service: Some(service_label.clone()),
        source: "database".to_owned(),
        logs,
        log_files: vec![],
    })
    .with_title(format!("MCP Logs (DB): {}{}", service_label, level_label)))
}

fn build_service_patterns(service: &str) -> Vec<String> {
    vec![format!("%{}%", service), format!("%rmcp%")]
}

fn build_all_mcp_patterns() -> Result<Vec<String>> {
    let services_config = ConfigLoader::load().context("Failed to load services config")?;

    let mut patterns: Vec<String> = services_config
        .mcp_servers
        .keys()
        .flat_map(|name| vec![format!("%{}%", name)])
        .collect();

    patterns.push("%rmcp%".to_owned());
    patterns.push("%mcp%".to_owned());

    Ok(patterns)
}
