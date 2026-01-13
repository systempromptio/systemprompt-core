mod cleanup;
mod delete;
pub mod request;
mod search;
mod stream;
pub mod trace;
mod view;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum LogsCommands {
    #[command(
        about = "View log entries",
        after_help = "EXAMPLES:\n  systemprompt logs view --tail 20\n  systemprompt logs view --level error\n  systemprompt logs view --since 1h"
    )]
    View(view::ViewArgs),

    #[command(
        about = "Search logs by pattern",
        after_help = "EXAMPLES:\n  systemprompt logs search \"error\"\n  systemprompt logs search \"timeout\" --level error --since 1h"
    )]
    Search(search::SearchArgs),

    #[command(
        about = "Stream logs in real-time",
        after_help = "EXAMPLES:\n  systemprompt logs stream\n  systemprompt logs stream --level error --module agent"
    )]
    Stream(stream::StreamArgs),

    #[command(about = "Clean up old log entries")]
    Cleanup(cleanup::CleanupArgs),

    #[command(about = "Delete all log entries")]
    Delete(delete::DeleteArgs),

    #[command(subcommand, about = "Debug execution traces")]
    Trace(trace::TraceCommands),

    #[command(subcommand, about = "Inspect AI requests")]
    Request(request::RequestCommands),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Output Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogEntryRow {
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

// ═══════════════════════════════════════════════════════════════════════════════
// Execute
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn execute(command: LogsCommands, config: &CliConfig) -> Result<()> {
    match command {
        LogsCommands::View(args) => view::execute(args, config).await,
        LogsCommands::Search(args) => search::execute(args, config).await,
        LogsCommands::Stream(args) => stream::execute(args, config).await,
        LogsCommands::Cleanup(args) => cleanup::execute(args, config).await,
        LogsCommands::Delete(args) => delete::execute(args, config).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd, config).await,
        LogsCommands::Request(cmd) => request::execute(cmd, config).await,
    }
}
