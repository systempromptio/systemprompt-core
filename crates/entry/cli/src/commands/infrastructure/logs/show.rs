use anyhow::{Result, anyhow};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};
use systemprompt_logging::{CliService, LogEntry, TraceQueryService};

use crate::CliConfig;
use crate::shared::{CommandResult, render_result};
use systemprompt_models::text::truncate_with_ellipsis;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Log entry ID or trace ID (can be partial)")]
    pub id: String,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogShowOutput {
    pub id: String,
    pub trace_id: String,
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceLogsOutput {
    pub trace_id: String,
    pub total: u64,
    pub logs: Vec<LogShowOutput>,
}

crate::define_pool_command!(ShowArgs => (), with_config);

async fn execute_with_pool_inner(
    args: ShowArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let service = TraceQueryService::new(Arc::clone(pool));

    if let Some(log) = service.find_log_by_id(&args.id).await? {
        display_single_log(&log, config, args.json);
        return Ok(());
    }

    let logs = service.find_logs_by_trace_id(&args.id).await?;
    if !logs.is_empty() {
        display_trace_logs(&logs, config, args.json);
        return Ok(());
    }

    if let Some(log) = service.find_log_by_partial_id(&args.id).await? {
        display_single_log(&log, config, args.json);
        return Ok(());
    }

    Err(anyhow!(
        "No log entries found for ID: {}. Try 'logs view' to see recent logs.",
        args.id
    ))
}

fn entry_to_output(entry: &LogEntry) -> LogShowOutput {
    LogShowOutput {
        id: entry.id.to_string(),
        trace_id: entry.trace_id.to_string(),
        timestamp: entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        level: entry.level.to_string().to_uppercase(),
        module: entry.module.clone(),
        message: entry.message.clone(),
        metadata: entry.metadata.clone(),
        user_id: Some(entry.user_id.clone()),
        session_id: Some(entry.session_id.clone()),
        task_id: entry.task_id.clone(),
        context_id: entry.context_id.clone(),
    }
}

fn display_single_log(log: &LogEntry, config: &CliConfig, json: bool) {
    let output = entry_to_output(log);

    if config.is_json_output() || json {
        let result = CommandResult::table(output).with_title("Log Entry Details");
        render_result(&result);
        return;
    }

    CliService::section("Log Entry Details");
    CliService::key_value("ID", &output.id);
    CliService::key_value("Trace ID", &output.trace_id);
    CliService::key_value("Timestamp", &output.timestamp);
    CliService::key_value("Level", &output.level);
    CliService::key_value("Module", &output.module);
    CliService::key_value("Message", &output.message);

    if let Some(user_id) = &output.user_id {
        CliService::key_value("User ID", user_id.as_str());
    }
    if let Some(session_id) = &output.session_id {
        CliService::key_value("Session ID", session_id.as_str());
    }
    if let Some(task_id) = &output.task_id {
        CliService::key_value("Task ID", task_id.as_str());
    }
    if let Some(context_id) = &output.context_id {
        CliService::key_value("Context ID", context_id.as_str());
    }

    if let Some(metadata) = &output.metadata {
        CliService::subsection("Metadata");
        if let Some(obj) = metadata.as_object() {
            for (key, value) in obj {
                let formatted = format!("{}", value).trim_matches('"').to_string();
                CliService::key_value(key, &formatted);
            }
        } else {
            CliService::info(&format!("{}", metadata));
        }
    }

    CliService::info("");
    CliService::info(&format!(
        "Tip: Use 'logs trace show {}' for full execution trace",
        truncate_with_ellipsis(&output.trace_id, 12)
    ));
}

fn display_trace_logs(logs: &[LogEntry], config: &CliConfig, json: bool) {
    let Some(first_log) = logs.first() else {
        return;
    };
    let trace_id = first_log.trace_id.to_string();
    let outputs: Vec<LogShowOutput> = logs.iter().map(entry_to_output).collect();

    let output = TraceLogsOutput {
        trace_id: trace_id.clone(),
        total: outputs.len() as u64,
        logs: outputs,
    };

    if config.is_json_output() || json {
        let result = CommandResult::table(output).with_title("Logs for Trace");
        render_result(&result);
        return;
    }

    CliService::section(&format!(
        "Logs for Trace: {}",
        truncate_with_ellipsis(&trace_id, 12)
    ));
    CliService::info(&format!("Found {} log entries", logs.len()));
    CliService::info("");

    for log in logs {
        let time_part = log.timestamp.format("%H:%M:%S%.3f").to_string();
        let level_str = log.level.to_string().to_uppercase();
        let line = format!(
            "{} {} [{}] {}",
            time_part, level_str, log.module, log.message
        );

        match level_str.as_str() {
            "ERROR" => CliService::error(&line),
            "WARN" => CliService::warning(&line),
            _ => CliService::info(&line),
        }
    }

    CliService::info("");
    CliService::info(&format!(
        "Tip: Use 'logs trace show {}' for full trace with AI/MCP details",
        truncate_with_ellipsis(&trace_id, 12)
    ));
}
