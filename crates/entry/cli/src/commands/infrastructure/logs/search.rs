use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::duration::parse_since;
use super::search_queries::{search_logs, search_tools};
use super::shared::display_log_row;
use super::{LogEntryRow, LogFilters};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(help = "Search pattern (matches message content and tool names)")]
    pub pattern: String,

    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name (partial match)")]
    pub module: Option<String>,

    #[arg(
        long,
        help = "Only search logs since this duration (e.g., '1h', '24h', '7d') or datetime"
    )]
    pub since: Option<String>,

    #[arg(long, short = 'n', default_value = "50", help = "Maximum results")]
    pub limit: i64,

    #[arg(long, default_value = "true", help = "Include MCP tool executions")]
    pub include_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolSearchResult {
    pub timestamp: String,
    pub trace_id: String,
    pub tool_name: String,
    pub server: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CombinedSearchOutput {
    pub logs: Vec<LogEntryRow>,
    pub log_count: u64,
    pub tools: Vec<ToolSearchResult>,
    pub tool_count: u64,
    pub filters: LogFilters,
}

pub async fn execute(args: SearchArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: SearchArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

async fn execute_with_pool_inner(
    args: SearchArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let level_filter = args.level.as_deref().map(str::to_uppercase);
    let pattern = format!("%{}%", args.pattern);

    let rows = search_logs(
        pool,
        &pattern,
        since_timestamp,
        level_filter.as_deref(),
        args.limit,
    )
    .await?;

    let tool_rows = if args.include_tools {
        search_tools(pool, &pattern, since_timestamp, args.limit).await?
    } else {
        vec![]
    };

    let filtered_rows: Vec<_> = match &args.module {
        Some(module) => rows
            .into_iter()
            .filter(|r| r.module.contains(module))
            .collect(),
        None => rows,
    };

    let logs: Vec<LogEntryRow> = filtered_rows
        .into_iter()
        .map(|r| LogEntryRow {
            id: r.id,
            trace_id: r.trace_id,
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: r.level.to_uppercase(),
            module: r.module,
            message: r.message,
            metadata: r.metadata.as_ref().and_then(|m| {
                serde_json::from_str(m)
                    .map_err(|e| {
                        tracing::warn!(error = %e, "Failed to parse log metadata");
                        e
                    })
                    .ok()
            }),
        })
        .collect();

    let tools: Vec<ToolSearchResult> = tool_rows
        .into_iter()
        .map(|r| ToolSearchResult {
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            trace_id: r.trace_id,
            tool_name: r.tool_name,
            server: r.server_name.unwrap_or_else(|| "unknown".to_string()),
            status: r.status,
            duration_ms: r.execution_time_ms.map(i64::from),
        })
        .collect();

    let filters = LogFilters {
        level: args.level.clone(),
        module: args.module.clone(),
        since: args.since.clone(),
        pattern: Some(args.pattern.clone()),
        tail: args.limit,
    };

    if config.is_json_output() {
        let output = CombinedSearchOutput {
            log_count: logs.len() as u64,
            logs,
            tool_count: tools.len() as u64,
            tools,
            filters,
        };
        let result = CommandResult::table(output).with_title("Search Results");
        render_result(&result);
    } else {
        render_combined_results(&logs, &tools, &args.pattern, &filters);
    }

    Ok(())
}

fn render_combined_results(
    logs: &[LogEntryRow],
    tools: &[ToolSearchResult],
    pattern: &str,
    filters: &LogFilters,
) {
    CliService::section(&format!("Search Results: \"{}\"", pattern));

    if filters.level.is_some() || filters.module.is_some() || filters.since.is_some() {
        if let Some(ref level) = filters.level {
            CliService::key_value("Level", level);
        }
        if let Some(ref module) = filters.module {
            CliService::key_value("Module", module);
        }
        if let Some(ref since) = filters.since {
            CliService::key_value("Since", since);
        }
    }

    if !tools.is_empty() {
        CliService::subsection(&format!("MCP Tool Executions ({})", tools.len()));
        for tool in tools {
            let duration = tool.duration_ms.map(|d| format!(" ({}ms)", d));
            let line = format!(
                "{} {}/{} [{}]{}  trace:{}",
                tool.timestamp,
                tool.server,
                tool.tool_name,
                tool.status,
                duration.as_deref().unwrap_or(""),
                tool.trace_id
            );
            match tool.status.as_str() {
                "error" | "failed" => CliService::error(&line),
                _ => CliService::info(&line),
            }
        }
    }

    if !logs.is_empty() {
        CliService::subsection(&format!("Log Entries ({})", logs.len()));
        for log in logs {
            display_log_row(log);
        }
    }

    if logs.is_empty() && tools.is_empty() {
        CliService::warning("No matching results found");
        return;
    }

    CliService::info(&format!(
        "Found {} log entries and {} tool executions",
        logs.len(),
        tools.len()
    ));
}
