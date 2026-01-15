use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

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
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceLogsOutput {
    pub trace_id: String,
    pub total: u64,
    pub logs: Vec<LogShowOutput>,
}

struct LogRow {
    id: String,
    trace_id: String,
    timestamp: DateTime<Utc>,
    level: String,
    module: String,
    message: String,
    metadata: Option<String>,
    user_id: Option<String>,
    session_id: Option<String>,
    task_id: Option<String>,
    context_id: Option<String>,
}

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    if let Some(log) = find_log_by_id(&pool, &args.id).await? {
        display_single_log(&log, config, args.json);
        return Ok(());
    }

    let logs = find_logs_by_trace(&pool, &args.id).await?;
    if !logs.is_empty() {
        display_trace_logs(&logs, config, args.json);
        return Ok(());
    }

    if let Some(log) = find_log_by_partial_id(&pool, &args.id).await? {
        display_single_log(&log, config, args.json);
        return Ok(());
    }

    Err(anyhow!(
        "No log entries found for ID: {}. Try 'logs view' to see recent logs.",
        args.id
    ))
}

async fn find_log_by_id(pool: &Arc<PgPool>, id: &str) -> Result<Option<LogRow>> {
    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT id as "id!", trace_id as "trace_id!", timestamp as "timestamp!",
               level as "level!", module as "module!", message as "message!",
               metadata, user_id, session_id, task_id, context_id
        FROM logs
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool.as_ref())
    .await?;

    Ok(row)
}

async fn find_log_by_partial_id(pool: &Arc<PgPool>, id: &str) -> Result<Option<LogRow>> {
    let pattern = format!("{}%", id);
    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT id as "id!", trace_id as "trace_id!", timestamp as "timestamp!",
               level as "level!", module as "module!", message as "message!",
               metadata, user_id, session_id, task_id, context_id
        FROM logs
        WHERE id LIKE $1
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        pattern
    )
    .fetch_optional(pool.as_ref())
    .await?;

    Ok(row)
}

async fn find_logs_by_trace(pool: &Arc<PgPool>, trace_id: &str) -> Result<Vec<LogRow>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT id as "id!", trace_id as "trace_id!", timestamp as "timestamp!",
               level as "level!", module as "module!", message as "message!",
               metadata, user_id, session_id, task_id, context_id
        FROM logs
        WHERE trace_id = $1
        ORDER BY timestamp ASC
        "#,
        trace_id
    )
    .fetch_all(pool.as_ref())
    .await?;

    if !rows.is_empty() {
        return Ok(rows);
    }

    let pattern = format!("{}%", trace_id);
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT id as "id!", trace_id as "trace_id!", timestamp as "timestamp!",
               level as "level!", module as "module!", message as "message!",
               metadata, user_id, session_id, task_id, context_id
        FROM logs
        WHERE trace_id LIKE $1
        ORDER BY timestamp ASC
        LIMIT 100
        "#,
        pattern
    )
    .fetch_all(pool.as_ref())
    .await?;

    Ok(rows)
}

fn row_to_output(row: &LogRow) -> LogShowOutput {
    LogShowOutput {
        id: row.id.clone(),
        trace_id: row.trace_id.clone(),
        timestamp: row.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        level: row.level.to_uppercase(),
        module: row.module.clone(),
        message: row.message.clone(),
        metadata: row.metadata.as_ref().and_then(|m| {
            serde_json::from_str(m)
                .map_err(|e| {
                    tracing::warn!(error = %e, "Failed to parse log metadata");
                    e
                })
                .ok()
        }),
        user_id: row.user_id.clone(),
        session_id: row.session_id.clone(),
        task_id: row.task_id.clone(),
        context_id: row.context_id.clone(),
    }
}

fn display_single_log(log: &LogRow, config: &CliConfig, json: bool) {
    let output = row_to_output(log);

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
        CliService::key_value("User ID", user_id);
    }
    if let Some(session_id) = &output.session_id {
        CliService::key_value("Session ID", session_id);
    }
    if let Some(task_id) = &output.task_id {
        CliService::key_value("Task ID", task_id);
    }
    if let Some(context_id) = &output.context_id {
        CliService::key_value("Context ID", context_id);
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
        truncate_id(&output.trace_id, 12)
    ));
}

fn display_trace_logs(logs: &[LogRow], config: &CliConfig, json: bool) {
    let trace_id = logs
        .first()
        .map(|l| l.trace_id.clone())
        .expect("display_trace_logs called with empty logs slice");
    let outputs: Vec<LogShowOutput> = logs.iter().map(row_to_output).collect();

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

    CliService::section(&format!("Logs for Trace: {}", truncate_id(&trace_id, 12)));
    CliService::info(&format!("Found {} log entries", logs.len()));
    CliService::info("");

    for log in logs {
        let time_part = log.timestamp.format("%H:%M:%S%.3f").to_string();
        let line = format!(
            "{} {} [{}] {}",
            time_part,
            log.level.to_uppercase(),
            log.module,
            log.message
        );

        match log.level.to_uppercase().as_str() {
            "ERROR" => CliService::error(&line),
            "WARN" => CliService::warning(&line),
            _ => CliService::info(&line),
        }
    }

    CliService::info("");
    CliService::info(&format!(
        "Tip: Use 'logs trace show {}' for full trace with AI/MCP details",
        truncate_id(&trace_id, 12)
    ));
}

fn truncate_id(id: &str, max_len: usize) -> String {
    if id.len() > max_len {
        format!("{}...", &id[..max_len])
    } else {
        id.to_string()
    }
}
