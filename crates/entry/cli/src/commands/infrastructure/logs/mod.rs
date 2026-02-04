mod audit;
mod audit_display;
mod cleanup;
mod delete;
pub mod duration;
mod export;
pub mod request;
mod search;
mod search_queries;
pub mod shared;
mod show;
mod stream;
mod summary;
pub mod tools;
pub mod trace;
pub mod types;
mod view;

pub use shared::{
    cost_microdollars_to_dollars, display_log_row, format_duration_ms, format_timestamp,
    truncate_id,
};
pub use types::{MessageRow, ToolCallRow};

use anyhow::{bail, Result};
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::DatabaseContext;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum LogsCommands {
    #[command(
        about = "View log entries",
        after_help = "EXAMPLES:\n  systemprompt infra logs view --tail 20\n  systemprompt infra \
                      logs view --level error\n  systemprompt infra logs view --since 1h"
    )]
    View(view::ViewArgs),

    #[command(
        about = "Search logs by pattern",
        after_help = "EXAMPLES:\n  systemprompt infra logs search \"error\"\n  systemprompt infra \
                      logs search \"timeout\" --level error --since 1h"
    )]
    Search(search::SearchArgs),

    #[command(
        about = "Stream logs in real-time (like tail -f)",
        visible_alias = "follow",
        after_help = "EXAMPLES:\n  systemprompt infra logs stream\n  systemprompt infra logs \
                      stream --level error --module agent\n  systemprompt infra logs follow"
    )]
    Stream(stream::StreamArgs),

    #[command(
        about = "Export logs to file",
        after_help = "EXAMPLES:\n  systemprompt infra logs export --format json --since 24h\n  \
                      systemprompt infra logs export --format csv -o logs.csv"
    )]
    Export(export::ExportArgs),

    #[command(about = "Clean up old log entries")]
    Cleanup(cleanup::CleanupArgs),

    #[command(about = "Delete all log entries")]
    Delete(delete::DeleteArgs),

    #[command(
        about = "Show logs summary statistics",
        after_help = "EXAMPLES:\n  systemprompt infra logs summary\n  systemprompt infra logs \
                      summary --since 24h"
    )]
    Summary(summary::SummaryArgs),

    #[command(
        about = "Show details of a log entry or all logs for a trace",
        after_help = "EXAMPLES:\n  systemprompt infra logs show log_abc123\n  systemprompt infra \
                      logs show trace_def456"
    )]
    Show(show::ShowArgs),

    #[command(subcommand, about = "Debug execution traces")]
    Trace(trace::TraceCommands),

    #[command(subcommand, about = "Inspect AI requests")]
    Request(request::RequestCommands),

    #[command(subcommand, about = "List and search MCP tool executions")]
    Tools(tools::ToolsCommands),

    #[command(
        about = "Full audit of an AI request with all messages and tool calls",
        after_help = "EXAMPLES:\n  systemprompt infra logs audit abc123\n  systemprompt infra \
                      logs audit abc123 --full\n  systemprompt infra logs audit task-xyz --json"
    )]
    Audit(audit::AuditArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogEntryRow {
    pub id: String,
    pub trace_id: String,
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub tail: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogViewOutput {
    pub logs: Vec<LogEntryRow>,
    pub total: u64,
    pub filters: LogFilters,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LogDeleteOutput {
    pub deleted_count: u64,
    pub vacuum_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogCleanupOutput {
    pub deleted_count: u64,
    pub dry_run: bool,
    pub cutoff_date: String,
    pub vacuum_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogExportOutput {
    pub exported_count: u64,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

pub async fn execute(command: LogsCommands, config: &CliConfig) -> Result<()> {
    match command {
        LogsCommands::View(args) => {
            let result = view::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        LogsCommands::Search(args) => {
            let result = search::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        LogsCommands::Stream(args) => stream::execute(args, config).await,
        LogsCommands::Export(args) => {
            let result = export::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        LogsCommands::Cleanup(args) => cleanup::execute(args, config).await,
        LogsCommands::Delete(args) => delete::execute(args, config).await,
        LogsCommands::Summary(args) => summary::execute(args, config).await,
        LogsCommands::Show(args) => show::execute(args, config).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd, config).await,
        LogsCommands::Request(cmd) => request::execute(cmd, config).await,
        LogsCommands::Tools(cmd) => tools::execute(cmd, config).await,
        LogsCommands::Audit(args) => audit::execute(args, config).await,
    }
}

pub async fn execute_with_db(
    command: LogsCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        LogsCommands::View(args) => {
            let result = view::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        LogsCommands::Search(args) => {
            let result = search::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        LogsCommands::Summary(args) => summary::execute_with_pool(args, db_ctx, config).await,
        LogsCommands::Export(args) => {
            let result = export::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        LogsCommands::Show(args) => show::execute_with_pool(args, db_ctx, config).await,
        LogsCommands::Trace(cmd) => trace::execute_with_pool(cmd, db_ctx, config).await,
        LogsCommands::Request(cmd) => request::execute_with_pool(cmd, db_ctx, config).await,
        LogsCommands::Tools(cmd) => tools::execute_with_pool(cmd, db_ctx, config).await,
        LogsCommands::Audit(args) => audit::execute_with_pool(args, db_ctx, config).await,
        LogsCommands::Stream(_) | LogsCommands::Cleanup(_) | LogsCommands::Delete(_) => {
            bail!("This logs command requires full profile context")
        },
    }
}
