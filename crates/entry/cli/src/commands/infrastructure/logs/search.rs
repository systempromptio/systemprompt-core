//! `infra logs search`: pattern-match log messages and (optionally) MCP tool
//! executions, returning a combined result set filtered by level, module, and
//! time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_database::pool_from_handle;
use systemprompt_identifiers::{McpServerId, McpToolName, TraceId};
use systemprompt_logging::LogLevel;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::{LogSearchItem, ToolExecutionItem, TraceQueryService, TraceRepository};

use super::duration::parse_since;
use super::shared::filter_notes;
use super::{LogEntryRow, LogFilters};
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(help = "Search pattern (matches message content and tool names)")]
    pub pattern: String,

    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<LogLevel>,

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
    pub trace_id: TraceId,
    pub tool_name: McpToolName,
    pub server: Option<McpServerId>,
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

pub(in crate::commands::infrastructure::logs) async fn execute(
    args: SearchArgs,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let pool = pool_from_handle(host.db().await?)?.pool();
    execute_with_pool_inner(args, &pool).await
}

async fn execute_with_pool_inner(
    args: SearchArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandOutput> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let pattern = format!("%{}%", args.pattern);

    let service = TraceQueryService::new(TraceRepository::new(Arc::clone(pool)));

    let rows = service
        .search_logs(&pattern, since_timestamp, args.level, args.limit)
        .await?;

    let tool_rows = if args.include_tools {
        service
            .search_tool_executions(&pattern, since_timestamp, args.limit)
            .await?
    } else {
        vec![]
    };

    let logs = map_log_rows(rows, args.module.as_deref());
    let tools = map_tool_rows(tool_rows);

    let filters = LogFilters {
        level: args.level,
        module: args.module.clone(),
        since: args.since.clone(),
        pattern: Some(args.pattern.clone()),
        tail: args.limit,
    };

    let output = CombinedSearchOutput {
        log_count: logs.len() as u64,
        logs,
        tool_count: tools.len() as u64,
        tools,
        filters,
    };

    Ok(search_output(&output, &args.pattern))
}

fn search_output(output: &CombinedSearchOutput, pattern: &str) -> CommandOutput {
    let table = CommandOutput::table_of(
        vec!["id", "trace_id", "timestamp", "level", "module", "message"],
        &output.logs,
    )
    .with_title(format!("Search Results: \"{pattern}\""));
    let mut notes = filter_notes(&output.filters);
    for tool in &output.tools {
        notes.push(tool_notice(tool));
    }
    if output.logs.is_empty() && output.tools.is_empty() {
        notes.push(NoticeLine::new("warning", "No matching results found"));
    } else {
        notes.push(NoticeLine::new(
            "info",
            format!(
                "Found {} log entries and {} tool executions",
                output.log_count, output.tool_count
            ),
        ));
    }
    notes.into_iter().fold(table, CommandOutput::with_note)
}

fn tool_notice(tool: &ToolSearchResult) -> NoticeLine {
    let duration = tool.duration_ms.map(|d| format!(" ({d}ms)"));
    let line = format!(
        "MCP {} {}/{} [{}]{}  trace:{}",
        tool.timestamp,
        tool.server.as_ref().map_or("unknown", McpServerId::as_str),
        tool.tool_name,
        tool.status,
        duration.as_deref().unwrap_or(""),
        tool.trace_id
    );
    match tool.status.as_str() {
        "error" | "failed" => NoticeLine::new("error", line),
        _ => NoticeLine::new("info", line),
    }
}

pub fn map_log_rows(rows: Vec<LogSearchItem>, module_filter: Option<&str>) -> Vec<LogEntryRow> {
    rows.into_iter()
        .filter(|r| module_filter.is_none_or(|module| r.module.contains(module)))
        .map(|r| LogEntryRow {
            id: r.id,
            trace_id: r.trace_id,
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: r.level,
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
        .collect()
}

pub fn map_tool_rows(rows: Vec<ToolExecutionItem>) -> Vec<ToolSearchResult> {
    rows.into_iter()
        .map(|r| ToolSearchResult {
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            trace_id: r.trace_id,
            tool_name: r.tool_name,
            server: r.server_name,
            status: r.status.as_str().to_owned(),
            duration_ms: r.execution_time_ms.map(i64::from),
        })
        .collect()
}
