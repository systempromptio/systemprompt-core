//! Database-log patterns for `plugins mcp logs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use systemprompt_database::{DbPool, pool_from_handle};
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_logging::LoggingRepository;
use systemprompt_manifest::ServicesConfig;

use super::logs::{LogLevel, LogsArgs};
use super::services_config;
use super::types::McpLogsOutput;

pub(super) async fn execute_db_mode(
    args: &LogsArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    let services = services_config(host).await?;
    let pool = pool_from_handle(host.db().await?)?;
    execute_db_mode_with_pool(args, &pool, &services).await
}

pub async fn execute_db_mode_with_pool(
    args: &LogsArgs,
    pool: &DbPool,
    services: &ServicesConfig,
) -> Result<CommandOutput> {
    let repo = LoggingRepository::new(pool);

    let patterns = args.server.as_ref().map_or_else(
        || build_all_mcp_patterns(services),
        |service| build_service_patterns(service),
    );

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

    let output = McpLogsOutput {
        service: Some(service_label.clone()),
        source: "database".to_owned(),
        logs,
        log_files: vec![],
    };

    Ok(CommandOutput::card_value(
        format!("MCP Logs (DB): {}{}", service_label, level_label),
        &output,
    ))
}

fn build_service_patterns(service: &str) -> Vec<String> {
    vec![format!("%{}%", service), "%rmcp%".to_owned()]
}

fn build_all_mcp_patterns(services: &ServicesConfig) -> Vec<String> {
    let mut patterns: Vec<String> = services
        .mcp_servers
        .keys()
        .flat_map(|name| vec![format!("%{}%", name)])
        .collect();

    patterns.push("%rmcp%".to_owned());
    patterns.push("%mcp%".to_owned());

    patterns
}
