//! Domain data types for the AI crate.
//!
//! Includes request records, message and tool-call rows, status enums, and
//! image-generation parameters used by the [`crate::services`] module. Vendor
//! wire shapes live in `systemprompt-wire`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{
    AiRequestId, AiToolCallId, ContextId, GatewayConversationId, McpExecutionId, McpToolName,
    ProviderRequestId, SessionId, TaskId, TraceId, UserId,
};

pub use systemprompt_models::ai as ai_models;

pub mod ai {
    pub use systemprompt_models::ai::{
        AiContentPart, AiMessage, AiRequest, AiResponse, GenerateResponseParams,
        GoogleSearchParams, MessageRole, ResponseFormat, SamplingParams, SearchGroundedResponse,
        StreamChunk, WebSource,
    };
}

pub mod tools {
    pub use systemprompt_models::ai::tools::{CallToolResult, McpTool, ToolCall};
}

pub mod ai_request_record;
mod ai_request_usage;
mod billing_source;
pub mod image_generation;
pub mod request_kind;
pub(crate) mod rows;
mod served_hop;

pub use ai_request_record::{AiRequestRecord, AiRequestRecordBuilder, CacheInfo, TokenInfo};
pub use billing_source::BillingSource;
pub use request_kind::{RequestKind, RequestStatus};
pub use served_hop::{CatalogHop, ServedHop};
pub use systemprompt_models::origin::{
    ClientAttestation, ClientEvidence, ClientKind, InboundWireProtocol, NativeMarker, RequestOrigin,
};

/// One persisted `ai_requests` row, grouped by concern; the serialised shape
/// stays flat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    #[serde(flatten)]
    pub identity: AiRequestIdentity,
    #[serde(flatten)]
    pub routing: AiRequestRouting,
    #[serde(flatten)]
    pub accounting: AiRequestAccounting,
    #[serde(flatten)]
    pub origin: AiRequestOrigin,
    #[serde(flatten)]
    pub lifecycle: AiRequestLifecycle,
}

/// The ids that place an [`AiRequest`] in its user, session, task and trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestIdentity {
    pub id: AiRequestId,
    pub request_id: AiRequestId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub task_id: Option<TaskId>,
    pub context_id: Option<ContextId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub provider_request_id: Option<ProviderRequestId>,
    pub trace_id: Option<TraceId>,
}

/// The provider, model and sampling an [`AiRequest`] was routed with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestRouting {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i32>,
    pub is_streaming: bool,
}

/// Token, cache, cost and latency counters of an [`AiRequest`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AiRequestAccounting {
    pub tokens_used: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub cache_hit: bool,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
}

/// The client kind, wire protocol and attestation an [`AiRequest`] arrived
/// with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestOrigin {
    pub client_kind: String,
    pub wire_protocol: String,
    pub client_attestation: String,
}

/// Status, error and timestamps of an [`AiRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestLifecycle {
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiRequestMessage {
    pub id: String,
    pub request_id: AiRequestId,
    pub role: String,
    pub content: String,
    pub sequence_number: i32,
    pub name: Option<String>,
    pub tool_call_id: Option<AiToolCallId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiRequestToolCall {
    pub id: String,
    pub request_id: AiRequestId,
    pub tool_name: McpToolName,
    pub tool_input: String,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub sequence_number: i32,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProviderUsage {
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAiUsage {
    #[sqlx(try_from = "String")]
    pub user_id: UserId,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_tokens_per_request: Option<f64>,
}
