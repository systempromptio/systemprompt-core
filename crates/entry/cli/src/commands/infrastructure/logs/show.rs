//! `infra logs show`: resolve an id to either a single log entry or all entries
//! sharing a trace id, falling back to a partial-id match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use systemprompt_database::pool_from_handle;
use anyhow::{Result, anyhow};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, LogId, SessionId, TaskId, TraceId, UserId};
use systemprompt_logging::{LogEntry, LogLevel};
use systemprompt_runtime::{TraceQueryService, TraceRepository};

use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Log entry ID or trace ID (can be partial)")]
    pub id: String,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct LogShowOutput {
    pub id: LogId,
    pub trace_id: TraceId,
    pub timestamp: String,
    #[schemars(with = "String")]
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // JSON: JSONB log `metadata` column — structured tracing fields, open-shaped.
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
}

pub(in crate::commands::infrastructure::logs) async fn execute(
    args: ShowArgs,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let pool = pool_from_handle(host.db().await?)?.pool();
    execute_with_pool_inner(args, &pool).await
}

async fn execute_with_pool_inner(
    args: ShowArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandOutput> {
    let service = TraceQueryService::new(TraceRepository::new(Arc::clone(pool)));

    if let Ok(log_id) = LogId::try_new(args.id.as_str())
        && let Some(log) = service.find_log_by_id(&log_id).await?
    {
        return Ok(single_log(&log));
    }

    if let Ok(trace_id) = TraceId::try_new(args.id.as_str()) {
        let logs = service.find_logs_by_trace_id(&trace_id).await?;
        if !logs.is_empty() {
            return Ok(trace_logs(&logs));
        }
    }

    if let Some(log) = service.find_log_by_partial_id(&args.id).await? {
        return Ok(single_log(&log));
    }

    Err(anyhow!(
        "No log entries found for ID: {}. Try 'logs view' to see recent logs.",
        args.id
    ))
}

fn entry_to_output(entry: &LogEntry) -> LogShowOutput {
    LogShowOutput {
        id: entry.id.clone(),
        trace_id: entry.trace_id.clone(),
        timestamp: entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        level: entry.level,
        module: entry.module.clone(),
        message: entry.message.clone(),
        metadata: entry.metadata.clone(),
        user_id: Some(entry.user_id.clone()),
        session_id: Some(entry.session_id.clone()),
        task_id: entry.task_id.clone(),
        context_id: entry.context_id.clone(),
    }
}

fn single_log(log: &LogEntry) -> CommandOutput {
    CommandOutput::card_value("Log Entry Details", &entry_to_output(log))
}

fn trace_logs(logs: &[LogEntry]) -> CommandOutput {
    let outputs: Vec<LogShowOutput> = logs.iter().map(entry_to_output).collect();
    CommandOutput::table_of(
        vec!["id", "trace_id", "timestamp", "level", "module", "message"],
        &outputs,
    )
    .with_title("Logs for Trace")
}
