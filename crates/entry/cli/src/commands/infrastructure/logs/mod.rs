//! `infra logs` command surface: querying, searching, and tracing the log
//! store.
//!
//! Dispatches the [`LogsCommands`] subcommands (view, search, stream, export,
//! cleanup, delete, summary, show, trace, request, tools, audit) and defines
//! the serializable output rows shared across them. On a `--database-url`
//! invocation only the read-only subcommands are served; stream, cleanup, and
//! delete require a full profile context.

mod audit;
mod cleanup;
pub(super) mod delete;
pub mod duration;
mod export;
pub mod request;
mod search;
pub mod shared;
mod show;
mod stream;
mod summary;
pub mod tools;
pub mod trace;
pub mod types;
mod view;

pub use audit::{AuditOutput, AuditToolCall, build_audit, not_found_output as audit_not_found};
pub use shared::{
    cost_microdollars_to_dollars, display_log_row, format_optional_duration_ms, format_timestamp,
};
pub use summary::{LogsSummaryOutput, build_logs_summary};
pub use types::{MessageRow, ToolCallRow};

use anyhow::{Result, bail};
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{LogId, TraceId};

use crate::context::CommandContext;
use crate::shared::render_result;

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
        about = "Full chain reconstruction by request, task, or trace id (identity, policy, prompt/response, tool calls, cost)",
        after_help = "EXAMPLES:\n  systemprompt infra logs audit abc123\n  systemprompt infra \
                      logs audit task-xyz"
    )]
    Audit(audit::AuditArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogEntryRow {
    pub id: LogId,
    pub trace_id: TraceId,
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

pub async fn execute(command: LogsCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(
            command,
            LogsCommands::Stream(_) | LogsCommands::Cleanup(_) | LogsCommands::Delete(_)
        )
    {
        bail!("This logs command requires full profile context");
    }

    match command {
        LogsCommands::View(args) => {
            let result = view::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        LogsCommands::Search(args) => {
            let result = search::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        LogsCommands::Stream(args) => stream::execute(args, ctx).await,
        LogsCommands::Export(args) => {
            let result = export::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        LogsCommands::Cleanup(args) => cleanup::execute(args, ctx).await,
        LogsCommands::Delete(args) => delete::execute(args, ctx).await,
        LogsCommands::Summary(args) => summary::execute(args, ctx).await,
        LogsCommands::Show(args) => show::execute(args, ctx).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd, ctx).await,
        LogsCommands::Request(cmd) => request::execute(cmd, ctx).await,
        LogsCommands::Tools(cmd) => tools::execute(cmd, ctx).await,
        LogsCommands::Audit(args) => audit::execute(args, ctx).await,
    }
}
