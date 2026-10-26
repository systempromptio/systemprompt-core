//! Private decode targets for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so rows decode into plain strings here and
//! become typed ids through the trusted `new` constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    AiRequestId, AiToolCallId, ContextId, GatewayConversationId, McpExecutionId, McpToolName,
    ProviderRequestId, SessionId, TaskId, TraceId, UserId,
};

use super::{
    AiRequest, AiRequestAccounting, AiRequestIdentity, AiRequestLifecycle, AiRequestOrigin,
    AiRequestRouting, AiRequestToolCall,
};

#[derive(Debug)]
pub(crate) struct AiRequestRow {
    pub id: AiRequestId,
    pub request_id: AiRequestId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub task_id: Option<TaskId>,
    pub context_id: Option<ContextId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub provider_request_id: Option<ProviderRequestId>,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i32>,
    pub tokens_used: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
    pub cache_hit: bool,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub is_streaming: bool,
    pub status: String,
    pub client_kind: String,
    pub wire_protocol: String,
    pub client_attestation: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

// Why: lint-ok: field-copy-from — the flat row regroups its columns into the
// identity, routing, accounting, origin and lifecycle parts.
impl From<AiRequestRow> for AiRequest {
    fn from(row: AiRequestRow) -> Self {
        Self {
            identity: AiRequestIdentity {
                id: row.id,
                request_id: row.request_id,
                user_id: row.user_id,
                session_id: row.session_id,
                task_id: row.task_id,
                context_id: row.context_id,
                gateway_conversation_id: row.gateway_conversation_id,
                provider_request_id: row.provider_request_id,
                trace_id: row.trace_id,
            },
            routing: AiRequestRouting {
                provider: row.provider,
                model: row.model,
                temperature: row.temperature,
                top_p: row.top_p,
                max_tokens: row.max_tokens,
                is_streaming: row.is_streaming,
            },
            accounting: AiRequestAccounting {
                tokens_used: row.tokens_used,
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                reasoning_tokens: row.reasoning_tokens,
                cache_hit: row.cache_hit,
                cache_read_tokens: row.cache_read_tokens,
                cache_creation_tokens: row.cache_creation_tokens,
                cost_microdollars: row.cost_microdollars,
                latency_ms: row.latency_ms,
                upstream_latency_ms: row.upstream_latency_ms,
            },
            origin: AiRequestOrigin {
                client_kind: row.client_kind,
                wire_protocol: row.wire_protocol,
                client_attestation: row.client_attestation,
            },
            lifecycle: AiRequestLifecycle {
                status: row.status,
                error_message: row.error_message,
                created_at: row.created_at,
                updated_at: row.updated_at,
                completed_at: row.completed_at,
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct AiRequestToolCallRow {
    pub id: String,
    pub request_id: AiRequestId,
    pub tool_name: String,
    pub tool_input: String,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub sequence_number: i32,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AiRequestToolCallRow> for AiRequestToolCall {
    fn from(row: AiRequestToolCallRow) -> Self {
        Self {
            id: row.id,
            request_id: row.request_id,
            tool_name: McpToolName::new(row.tool_name),
            tool_input: row.tool_input,
            mcp_execution_id: row.mcp_execution_id,
            sequence_number: row.sequence_number,
            ai_tool_call_id: row.ai_tool_call_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
