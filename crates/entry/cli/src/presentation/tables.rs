//! Reusable `tabled` table widgets for command output.
//!
//! Pure shaping and rendering: each function turns domain records into a
//! rendered table string. Callers decide where the string is printed, so the
//! row shaping stays testable without a terminal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_logging::{
    AiRequestInfo, ExecutionStep, McpToolExecution, TaskArtifact, TaskInfo, TraceEvent,
};
use tabled::settings::Style;
use tabled::{Table, Tabled};

use crate::commands::core::artifacts::ArtifactSummary;
use crate::commands::core::contexts::ContextSummary;
use crate::commands::infrastructure::db::TableInfo;
use crate::shared::truncate_with_ellipsis;

#[must_use]
pub fn truncate_cell(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").replace('\r', "");
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s
    }
}

fn dash() -> String {
    "-".to_owned()
}

fn millis(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(dash, |ms| format!("{ms}ms"))
}

#[derive(Tabled)]
struct ArtifactListRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    artifact_type: String,
    #[tabled(rename = "Tool")]
    tool_name: String,
    #[tabled(rename = "Created")]
    created_at: String,
}

#[must_use]
pub fn artifact_list_table(artifacts: &[ArtifactSummary]) -> String {
    let rows: Vec<ArtifactListRow> = artifacts
        .iter()
        .map(|a| ArtifactListRow {
            id: truncate_with_ellipsis(a.artifact_id.as_str(), 12),
            name: a.name.clone().unwrap_or_else(dash),
            artifact_type: a.artifact_type.clone(),
            tool_name: a.tool_name.clone().unwrap_or_else(dash),
            created_at: a.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct ContextListRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Tasks")]
    task_count: i64,
    #[tabled(rename = "Messages")]
    message_count: i64,
    #[tabled(rename = "Updated")]
    updated_at: String,
    #[tabled(rename = "Active")]
    active: String,
}

#[must_use]
pub fn context_list_table(contexts: &[ContextSummary]) -> String {
    let rows: Vec<ContextListRow> = contexts
        .iter()
        .map(|c| ContextListRow {
            id: c.id.as_str().chars().take(8).collect(),
            name: truncate_with_ellipsis(&c.name, 40),
            task_count: c.task_count,
            message_count: c.message_count,
            updated_at: c.updated_at.format("%Y-%m-%d %H:%M").to_string(),
            active: if c.is_active {
                "*".to_owned()
            } else {
                String::new()
            },
        })
        .collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct DbTableRow {
    #[tabled(rename = "Table")]
    name: String,
    #[tabled(rename = "Rows")]
    row_count: i64,
    #[tabled(rename = "Size")]
    size: String,
}

#[must_use]
pub fn db_tables_table(tables: &[TableInfo]) -> String {
    let rows: Vec<DbTableRow> = tables
        .iter()
        .map(|t| DbTableRow {
            name: t.name.clone(),
            row_count: t.row_count,
            size: crate::commands::infrastructure::db::format_bytes(t.size_bytes),
        })
        .collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct TaskInfoRow {
    #[tabled(rename = "Task ID")]
    task: String,
    #[tabled(rename = "Agent")]
    agent_name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Started")]
    started_at: String,
    #[tabled(rename = "Duration")]
    duration: String,
}

#[must_use]
pub fn task_info_table(task_info: &TaskInfo) -> String {
    let rows = vec![TaskInfoRow {
        task: task_info.task_id.as_str().chars().take(8).collect(),
        agent_name: task_info.agent_name.clone().unwrap_or_else(dash),
        status: task_info.status.clone(),
        started_at: task_info
            .started_at
            .map_or_else(dash, |t| t.format("%H:%M:%S").to_string()),
        duration: millis(task_info.execution_time_ms),
    }];
    Table::new(rows).with(Style::rounded()).to_string()
}

#[derive(Tabled)]
struct StepRow {
    #[tabled(rename = "#")]
    step_number: usize,
    #[tabled(rename = "Type")]
    step_type: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Duration")]
    duration: String,
}

#[must_use]
pub fn execution_steps_table(steps: &[ExecutionStep]) -> String {
    let rows: Vec<StepRow> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| StepRow {
            step_number: i + 1,
            step_type: s.step_type.clone().unwrap_or_else(|| "unknown".to_owned()),
            title: truncate_cell(s.title.as_deref().unwrap_or_default(), 40),
            status: s.status.clone(),
            duration: millis(s.duration_ms),
        })
        .collect();
    Table::new(rows).with(Style::rounded()).to_string()
}

#[derive(Tabled)]
struct AiRequestRow {
    #[tabled(rename = "Model")]
    model: String,
    #[tabled(rename = "Max")]
    max_tokens: String,
    #[tabled(rename = "Tokens")]
    tokens: String,
    #[tabled(rename = "Cost")]
    cost: String,
    #[tabled(rename = "Latency")]
    latency: String,
}

#[must_use]
pub fn ai_requests_table(requests: &[AiRequestInfo]) -> String {
    let rows: Vec<AiRequestRow> = requests
        .iter()
        .map(|r| AiRequestRow {
            model: format!("{}/{}", r.provider, r.model),
            max_tokens: r.max_tokens.map_or_else(dash, |t| t.to_string()),
            tokens: format!(
                "{} (in:{}, out:{})",
                r.input_tokens.unwrap_or(0) + r.output_tokens.unwrap_or(0),
                r.input_tokens.unwrap_or(0),
                r.output_tokens.unwrap_or(0)
            ),
            #[expect(
                clippy::cast_precision_loss,
                reason = "display-only dollar conversion of microdollar totals"
            )]
            cost: format!("${:.4}", r.cost_microdollars as f64 / 1_000_000.0),
            latency: millis(r.latency_ms),
        })
        .collect();
    Table::new(rows).with(Style::rounded()).to_string()
}

#[derive(Tabled)]
struct ToolCallRow {
    #[tabled(rename = "Tool")]
    tool_name: String,
    #[tabled(rename = "Server")]
    server: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Duration")]
    duration: String,
}

#[must_use]
pub fn mcp_tool_calls_table(executions: &[McpToolExecution]) -> String {
    let rows: Vec<ToolCallRow> = executions
        .iter()
        .map(|e| ToolCallRow {
            tool_name: e.tool_name.clone(),
            server: e.server_name.clone(),
            status: e.status.clone(),
            duration: millis(e.execution_time_ms),
        })
        .collect();
    Table::new(rows).with(Style::rounded()).to_string()
}

#[derive(Tabled)]
struct TaskArtifactRow {
    #[tabled(rename = "ID")]
    artifact: String,
    #[tabled(rename = "Type")]
    artifact_type: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Source")]
    source: String,
    #[tabled(rename = "Tool")]
    tool_name: String,
}

#[must_use]
pub fn task_artifacts_table(artifacts: &[TaskArtifact]) -> String {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let rows: Vec<TaskArtifactRow> = artifacts
        .iter()
        .filter(|a| seen.insert(a.artifact_id.to_string()))
        .map(|a| TaskArtifactRow {
            artifact: truncate_cell(a.artifact_id.as_str(), 12),
            artifact_type: a.artifact_type.clone(),
            name: a.name.as_ref().map_or_else(dash, |s| truncate_cell(s, 30)),
            source: a.source.clone().unwrap_or_else(dash),
            tool_name: a.tool_name.clone().unwrap_or_else(dash),
        })
        .collect();
    Table::new(&rows).with(Style::rounded()).to_string()
}

#[derive(Tabled)]
struct TraceRow {
    #[tabled(rename = "Time")]
    time: String,
    #[tabled(rename = "Delta")]
    delta: String,
    #[tabled(rename = "Type")]
    event_type: String,
    #[tabled(rename = "Details")]
    details: String,
    #[tabled(rename = "Latency")]
    latency: String,
}

#[must_use]
pub fn format_metadata_value(key: &str, value: &serde_json::Value) -> String {
    let raw = || format!("{value}").trim_matches('"').to_owned();
    match key {
        "cost_microdollars" => value.as_i64().map_or_else(raw, |microdollars| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "display-only dollar conversion of microdollar totals"
            )]
            let dollars = microdollars as f64 / 1_000_000.0;
            format!("${dollars:.6}")
        }),
        "latency_ms" | "execution_time_ms" => {
            value.as_i64().map_or_else(raw, |ms| format!("{ms}ms"))
        },
        "tokens_used" => value.as_i64().map_or_else(raw, |tokens| tokens.to_string()),
        _ => raw(),
    }
}

#[must_use]
pub fn extract_latency_from_metadata(metadata: Option<&str>, event_type: &str) -> String {
    if let Some(meta) = metadata
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(meta)
    {
        let key = match event_type {
            "AI" => Some("latency_ms"),
            "MCP" => Some("execution_time_ms"),
            _ => None,
        };
        if let Some(key) = key
            && let Some(ms) = parsed.get(key).and_then(serde_json::Value::as_i64)
        {
            return format!("{ms}ms");
        }
    }
    dash()
}

#[must_use]
pub fn trace_events_table(events: &[TraceEvent]) -> String {
    let mut prev_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;
    let rows: Vec<TraceRow> = events
        .iter()
        .map(|e| {
            let delta = prev_timestamp.map_or_else(
                || "+0ms".to_owned(),
                |prev| {
                    let delta_ms = e.timestamp.signed_duration_since(prev).num_milliseconds();
                    format!("+{delta_ms}ms")
                },
            );
            prev_timestamp = Some(e.timestamp);
            TraceRow {
                time: e.timestamp.format("%H:%M:%S%.3f").to_string(),
                delta,
                event_type: e.event_type.clone(),
                details: truncate_cell(&e.details, 100),
                latency: extract_latency_from_metadata(e.metadata.as_deref(), &e.event_type),
            }
        })
        .collect();

    if rows.is_empty() {
        String::new()
    } else {
        Table::new(rows).with(Style::modern()).to_string()
    }
}
