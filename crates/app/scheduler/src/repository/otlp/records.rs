//! Audit rows as the exporter reads them, with their identities typed.
//!
//! `GovernanceRow::tool_name` stays a string: the `governance_decisions`
//! column mixes tool names, entity ids and a merge label, so it is not an
//! [`systemprompt_identifiers::McpToolName`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    AiRequestId, AiToolCallId, ClientId, ContextId, GatewayConversationId, InstanceId,
    McpExecutionId, McpServerId, McpToolName, PluginId, ProviderRequestId, SessionId, TraceId,
    UserId,
};

#[derive(Debug, Clone)]
pub struct RequestRow {
    pub id: AiRequestId,
    pub request_id: AiRequestId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub context_id: ContextId,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub served_provider: Option<String>,
    pub model: Option<String>,
    pub requested_model: Option<String>,
    pub route_match: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
    pub finish_reason: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub client_kind: String,
    pub wire_protocol: String,
    pub request_kind: String,
    pub actor_kind: String,
    pub actor_id: String,
    pub instance_id: Option<InstanceId>,
    pub created_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LedgerRow {
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub request_id: Option<AiRequestId>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub tool_name: Option<McpToolName>,
    pub server_name: Option<McpServerId>,
    pub intended_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i32>,
    pub execution_status: Option<String>,
    pub error_message: Option<String>,
    pub source: Option<String>,
    pub state: Option<String>,
    pub is_error: Option<bool>,
    pub artifact_type: Option<String>,
    pub payload_bytes: Option<i32>,
    pub secret_redactions: Option<i32>,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct GovernanceRow {
    pub id: String,
    pub trace_id: Option<TraceId>,
    pub tool_name: String,
    pub decision: String,
    pub policy: String,
    pub reason: String,
    pub plugin_id: Option<PluginId>,
    pub actor_kind: String,
    pub actor_id: String,
    pub tool_use_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LogRow {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub client_id: Option<ClientId>,
    pub instance_id: Option<InstanceId>,
    pub provider_request_id: Option<ProviderRequestId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
}
