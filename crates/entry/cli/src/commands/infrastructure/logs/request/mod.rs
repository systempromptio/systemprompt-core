//! `infra logs request` subcommands for inspecting AI provider requests.
//!
//! Exposes [`RequestCommands`] (list, show, stats) and the row types
//! ([`RequestListRow`], [`RequestShowOutput`]) returned to the renderer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod show;
mod stats;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{MessageRow, ToolCallRow};
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::UserId;
use systemprompt_models::artifacts::{Column, ColumnType, NoticeLine, TableArtifact};

pub use stats::{RequestStatsOutput, build_request_stats};

const REQUEST_ID_DISPLAY_WIDTH: usize = 12;

fn request_list_columns() -> Vec<Column> {
    vec![
        Column::new("request_id", ColumnType::String).with_width(REQUEST_ID_DISPLAY_WIDTH),
        Column::new("timestamp", ColumnType::String),
        Column::new("user_id", ColumnType::String),
        Column::new("actor", ColumnType::String),
        Column::new("client", ColumnType::String),
        Column::new("attestation", ColumnType::String),
        Column::new("provider", ColumnType::String),
        Column::new("model", ColumnType::String),
        Column::new("tokens", ColumnType::String),
        Column::new("cost", ColumnType::String),
        Column::new("latency_ms", ColumnType::Number),
        Column::new("status", ColumnType::String),
        Column::new("finish", ColumnType::String),
    ]
}

#[must_use]
pub fn build_request_list(rows: &[RequestListRow]) -> CommandOutput {
    if rows.is_empty() {
        return CommandOutput::message(vec![NoticeLine::new("info", "No AI requests found")]);
    }
    let items: Vec<JsonValue> = rows
        .iter()
        .map(|row| serde_json::to_value(row).unwrap_or(JsonValue::Null))
        .collect();
    CommandOutput::table_artifact(TableArtifact::new(request_list_columns()).with_rows(items))
        .with_title("AI Requests")
}

#[must_use]
pub fn build_request_show(detail: &RequestShowOutput) -> CommandOutput {
    CommandOutput::card_value("AI Request Details", detail)
}

#[must_use]
pub fn request_show_not_found(request_id: &str) -> CommandOutput {
    CommandOutput::message(vec![
        NoticeLine::new("warning", format!("AI request not found: {request_id}")),
        NoticeLine::new(
            "info",
            "Tip: Use 'systemprompt infra logs request list' to see recent requests",
        ),
    ])
}

#[derive(Debug, Subcommand)]
pub enum RequestCommands {
    #[command(
        about = "Operational list of recent AI requests. For dashboard metrics (time range, model filter, CSV export), use `analytics requests list`",
        after_help = "EXAMPLES:\n  systemprompt infra logs request list\n  systemprompt infra \
                      logs request list --model gpt-4 --since 1h"
    )]
    List(list::ListArgs),

    #[command(
        about = "Quick single-request view by request id (messages, linked MCP calls, status/error)",
        after_help = "EXAMPLES:\n  systemprompt infra logs request show abc123\n  systemprompt \
                      infra logs request show abc123 --messages --tools"
    )]
    Show(show::ShowArgs),

    #[command(
        about = "Operational request aggregate with by-provider / by-model breakdown. For range/model-filtered dashboards with export, use `analytics requests stats`",
        after_help = "EXAMPLES:\n  systemprompt infra logs request stats\n  systemprompt infra \
                      logs request stats --since 24h"
    )]
    Stats(stats::StatsArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestListRow {
    pub request_id: String,
    pub timestamp: String,
    pub cursor: String,
    pub user_id: UserId,
    pub actor: String,
    pub client: String,
    pub attestation: String,
    pub provider: String,
    pub model: String,
    pub tokens: String,
    pub cost: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
    pub status: String,
    /// The upstream's own finish reason, unnormalised; `-` before the terminal
    /// event or for rows older than the column.
    pub finish: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestShowOutput {
    pub request_id: String,
    pub user_id: UserId,
    pub actor_kind: String,
    pub actor_id: String,
    pub client: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_evidence: Option<ClientEvidenceOutput>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_dollars: f64,
    pub latency_ms: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub messages: Vec<MessageRow>,
    pub linked_mcp_calls: Vec<ToolCallRow>,
}

/// Only the evidence the wire presented is rendered; a field the request did
/// not carry is omitted rather than shown empty.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClientEvidenceOutput {
    pub kind_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attested_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_client: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_marker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ua_product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ua_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_package_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_runtime_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_arch: Option<String>,
}

impl From<systemprompt_runtime::AiRequestClientEvidence> for ClientEvidenceOutput {
    fn from(evidence: systemprompt_runtime::AiRequestClientEvidence) -> Self {
        Self {
            kind_source: evidence.kind_source,
            attested_host: evidence.attested_host,
            declared_client: evidence.declared_client,
            native_marker: evidence.native_marker,
            ua_product: evidence.ua_product,
            ua_version: evidence.ua_version,
            sdk_lang: evidence.sdk_lang,
            sdk_package_version: evidence.sdk_package_version,
            sdk_runtime: evidence.sdk_runtime,
            sdk_runtime_version: evidence.sdk_runtime_version,
            sdk_os: evidence.sdk_os,
            sdk_arch: evidence.sdk_arch,
        }
    }
}

pub async fn execute(command: RequestCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        RequestCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestCommands::Show(args) => {
            let result = show::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestCommands::Stats(args) => stats::execute(args, ctx).await,
    }
}
