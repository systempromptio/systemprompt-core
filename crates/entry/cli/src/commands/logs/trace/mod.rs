mod ai_artifacts;
mod ai_display;
mod ai_mcp;
mod ai_trace;
mod client;
mod display;
mod json;
mod list;
mod lookup;
mod summary;
mod viewer;

pub use summary::{print_summary, SummaryContext};

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CliConfig;

pub use viewer::TraceOptions;

// ═══════════════════════════════════════════════════════════════════════════════
// Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
pub enum TraceCommands {
    #[command(about = "View trace for a message or trace ID")]
    View {
        trace_id: Option<String>,
        #[command(flatten)]
        options: TraceOptions,
    },

    #[command(about = "AI task trace - inspect task execution details")]
    Ai(ai_trace::AiTraceOptions),

    #[command(about = "List recent traces")]
    List(list::ListArgs),

    #[command(about = "Lookup a specific AI request")]
    Lookup(lookup::LookupArgs),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Output Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceEventRow {
    pub timestamp: String,
    pub delta_ms: i64,
    pub event_type: String,
    pub details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiSummaryRow {
    pub request_count: i64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_dollars: f64,
    pub total_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpSummaryRow {
    pub execution_count: i64,
    pub total_execution_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StepSummaryRow {
    pub total: i64,
    pub completed: i64,
    pub failed: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceViewOutput {
    pub trace_id: String,
    pub events: Vec<TraceEventRow>,
    pub ai_summary: AiSummaryRow,
    pub mcp_summary: McpSummaryRow,
    pub step_summary: StepSummaryRow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceListRow {
    pub trace_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub ai_requests: i64,
    pub mcp_calls: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceListOutput {
    pub traces: Vec<TraceListRow>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskInfoRow {
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StepRow {
    pub step_number: i32,
    pub step_type: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiRequestRow {
    pub request_id: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    pub tokens: String,
    pub cost: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallRow {
    pub tool_name: String,
    pub server: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactRow {
    pub artifact_id: String,
    pub artifact_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiTraceOutput {
    pub task_id: String,
    pub task_info: TaskInfoRow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_input: Option<String>,
    pub execution_steps: Vec<StepRow>,
    pub ai_requests: Vec<AiRequestRow>,
    pub mcp_executions: Vec<ToolCallRow>,
    pub artifacts: Vec<ArtifactRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageRow {
    pub sequence: i32,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiLookupOutput {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_dollars: f64,
    pub latency_ms: i64,
    pub messages: Vec<MessageRow>,
    pub linked_mcp_calls: Vec<ToolCallRow>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Execute
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn execute(command: TraceCommands, config: &CliConfig) -> Result<()> {
    match command {
        TraceCommands::View { trace_id, options } => {
            viewer::execute(trace_id.as_deref(), options, config).await
        }
        TraceCommands::Ai(options) => ai_trace::execute(options, config).await,
        TraceCommands::List(args) => list::execute(args, config).await,
        TraceCommands::Lookup(args) => lookup::execute(args, config).await,
    }
}
