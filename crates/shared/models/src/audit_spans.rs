//! Audit facts as typed spans: the request, tool-call and governance records
//! a producer hands to the telemetry exporter at the moment it records them.
//!
//! Producers (gateway settlement, the MCP execution ledger, the governance
//! decision writer) build one record per fact and pass it to an
//! [`AuditSpanSink`] injected at the composition root. Handing a record over
//! never blocks and never fails the caller: the sink is bounded and drops
//! what it cannot queue. No record carries secret material; identifiers,
//! names, counters and timings only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    Actor, AiRequestId, AiToolCallId, ApiKeyId, ContextId, GovernanceDecisionId, InstanceId,
    McpExecutionId, McpServerId, McpToolName, PluginId, SessionId, ToolUseId, TraceId, UserId,
};
use systemprompt_traits::RequestStatus;

use crate::attribution::AttributionEntry;

#[derive(Debug, Clone)]
pub struct RequestSpanRecord {
    pub request_id: AiRequestId,
    pub request_kind: String,
    pub status: RequestStatus,
    pub error_message: Option<String>,
    pub user_id: UserId,
    pub actor: Actor,
    pub session_id: Option<SessionId>,
    pub context_id: ContextId,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub served_provider: Option<String>,
    pub requested_model: Option<String>,
    pub model: Option<String>,
    pub route_match: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
    pub finish_reason: Option<String>,
    pub client_kind: String,
    pub wire_protocol: String,
    pub instance_id: Option<InstanceId>,
    pub api_key_id: Option<ApiKeyId>,
    pub attributions: Vec<AttributionEntry>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ToolCallSpanRecord {
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub request_id: Option<AiRequestId>,
    pub trace_id: Option<TraceId>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub tool_name: Option<McpToolName>,
    pub server_name: Option<McpServerId>,
    pub state: Option<String>,
    pub source: Option<String>,
    pub execution_status: Option<String>,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub artifact_type: Option<String>,
    pub payload_bytes: Option<i32>,
    pub secret_redactions: Option<i32>,
    pub is_error: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GovernanceSpanRecord {
    pub id: GovernanceDecisionId,
    pub request_id: Option<AiRequestId>,
    pub trace_id: Option<TraceId>,
    pub decision: String,
    pub policy: String,
    pub reason: String,
    pub tool_name: String,
    pub tool_use_id: Option<ToolUseId>,
    pub plugin_id: Option<PluginId>,
    pub actor: Actor,
    pub decided_at: DateTime<Utc>,
}

/// Where producers hand the audit spans they record.
///
/// Implemented by the logging crate's sink, which offers them to the OTLP
/// exporter when one is running and discards them otherwise. Every method is
/// synchronous and non-blocking.
pub trait AuditSpanSink: Send + Sync + std::fmt::Debug {
    fn request(&self, span: RequestSpanRecord);
    fn tool_call(&self, span: ToolCallSpanRecord);
    fn governance(&self, span: GovernanceSpanRecord);
}

pub type DynAuditSpanSink = Arc<dyn AuditSpanSink>;
