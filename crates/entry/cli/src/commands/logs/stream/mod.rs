mod cleanup;
mod delete;
mod view;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CliConfig;

#[derive(Subcommand)]
pub enum StreamCommands {
    #[command(about = "View log entries")]
    View(view::ViewArgs),

    #[command(about = "Delete all log entries")]
    Delete(delete::DeleteArgs),

    #[command(about = "Clean up old log entries")]
    Cleanup(cleanup::CleanupArgs),
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
    pub tail: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogViewOutput {
    pub logs: Vec<LogEntryRow>,
    pub total: u64,
    pub filters: LogFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

pub async fn execute(cmd: StreamCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        StreamCommands::View(args) => view::execute(args, config).await,
        StreamCommands::Delete(args) => delete::execute(args, config).await,
        StreamCommands::Cleanup(args) => cleanup::execute(args, config).await,
    }
}
