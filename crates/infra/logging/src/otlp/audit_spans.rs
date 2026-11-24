//! Audit span records → OTLP spans: one `ai_request` span per settled
//! gateway request, a `tool_call` span per recorded MCP execution and a
//! `governance_decision` span per recorded decision. The attribute names are
//! the ones the audit-trail replay emitted, so dashboards built on it read
//! the live export unchanged. Pure: no I/O.
//!
//! A child span names the request span as its parent when its producer knows
//! the request; otherwise it is a root span of the same trace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::status::StatusCode;
use opentelemetry_proto::tonic::trace::v1::{Span, Status};
use systemprompt_identifiers::{
    AiRequestId, AiToolCallId, ApiKeyId, InstanceId, McpExecutionId, McpServerId, McpToolName,
    PluginId, SessionId, TraceId,
};
use systemprompt_models::audit_spans::{
    GovernanceSpanRecord, RequestSpanRecord, ToolCallSpanRecord,
};
use systemprompt_traits::RequestStatus;

use super::attrs::Attrs;
use super::ids::{span_id_bytes, trace_id_bytes, unix_nanos};

pub const REQUEST_SPAN: &str = "ai_request";
pub const TOOL_SPAN: &str = "tool_call";
pub const GOVERNANCE_SPAN: &str = "governance_decision";

fn trace_key<'a>(trace: Option<&'a TraceId>, fallback: &'a str) -> &'a str {
    trace
        .map(TraceId::as_str)
        .filter(|t| !t.is_empty())
        .unwrap_or(fallback)
}

fn parent_of(request: Option<&AiRequestId>) -> Vec<u8> {
    request.map_or_else(Vec::new, |id| span_id_bytes(REQUEST_SPAN, id.as_str()))
}

fn request_attrs(record: &RequestSpanRecord) -> Attrs {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.request.id", record.request_id.as_str())
        .text("systemprompt.request.kind", &record.request_kind)
        .text("systemprompt.request.status", record.status.as_str())
        .text("enduser.id", record.user_id.as_str())
        .text("systemprompt.actor.kind", &record.actor_kind)
        .text("systemprompt.actor.id", &record.actor_id)
        .opt_str(
            "session.id",
            record.session_id.as_ref().map(SessionId::as_str),
        )
        .text("systemprompt.context.id", record.context_id.as_str())
        .opt_str(
            "systemprompt.trace.id",
            record.trace_id.as_ref().map(TraceId::as_str),
        )
        .opt_str("gen_ai.system", record.provider.as_deref())
        .opt_str(
            "systemprompt.served_provider",
            record.served_provider.as_deref(),
        )
        .opt_str("gen_ai.request.model", record.requested_model.as_deref())
        .opt_str("gen_ai.response.model", record.model.as_deref())
        .opt_str("systemprompt.route", record.route_match.as_deref())
        .opt_int("gen_ai.usage.input_tokens", record.input_tokens)
        .opt_int("gen_ai.usage.output_tokens", record.output_tokens)
        .opt_int("gen_ai.usage.cache_read_tokens", record.cache_read_tokens)
        .opt_int(
            "gen_ai.usage.cache_creation_tokens",
            record.cache_creation_tokens,
        )
        .float(
            "systemprompt.cost.usd",
            record.cost_microdollars as f64 / 1_000_000.0,
        )
        .int("systemprompt.cost.microdollars", record.cost_microdollars)
        .opt_int("systemprompt.latency_ms", record.latency_ms)
        .opt_int(
            "systemprompt.upstream_latency_ms",
            record.upstream_latency_ms,
        )
        .opt_str(
            "gen_ai.response.finish_reason",
            record.finish_reason.as_deref(),
        )
        .text("systemprompt.client.kind", &record.client_kind)
        .text("systemprompt.wire_protocol", &record.wire_protocol)
        .opt_str(
            "systemprompt.instance.id",
            record.instance_id.as_ref().map(InstanceId::as_str),
        )
        .opt_str(
            "systemprompt.api_key.id",
            record.api_key_id.as_ref().map(ApiKeyId::as_str),
        )
        .scopes(&record.attributions);
    attrs
}

#[must_use]
pub fn request_span(record: &RequestSpanRecord) -> Span {
    let (code, message) = match record.status {
        RequestStatus::Completed => (StatusCode::Ok, String::new()),
        RequestStatus::Pending => (StatusCode::Unset, String::new()),
        RequestStatus::Failed | RequestStatus::Rejected => (
            StatusCode::Error,
            record
                .error_message
                .clone()
                .unwrap_or_else(|| record.status.as_str().to_owned()),
        ),
    };
    let key = record.request_id.as_str();
    Span {
        trace_id: trace_id_bytes(trace_key(record.trace_id.as_ref(), key)),
        span_id: span_id_bytes(REQUEST_SPAN, key),
        name: format!(
            "{REQUEST_SPAN} {}",
            record.model.as_deref().unwrap_or("unrouted")
        ),
        kind: SpanKind::Server as i32,
        start_time_unix_nano: unix_nanos(record.started_at),
        end_time_unix_nano: unix_nanos(record.completed_at.max(record.started_at)),
        attributes: request_attrs(record).finish(),
        status: Some(Status {
            code: code as i32,
            message,
        }),
        ..Default::default()
    }
}

#[must_use]
pub fn tool_call_span(record: &ToolCallSpanRecord) -> Span {
    let tool_name = record.tool_name.as_ref().map(McpToolName::as_str);
    let call_id = record.ai_tool_call_id.as_ref().map(AiToolCallId::as_str);
    let execution_id = record.mcp_execution_id.as_ref().map(McpExecutionId::as_str);
    let key = call_id.or(execution_id).unwrap_or_default();
    let mut attrs = Attrs::new();
    attrs
        .opt_str("gen_ai.tool.name", tool_name)
        .opt_str("gen_ai.tool.call.id", call_id)
        .opt_str(
            "systemprompt.mcp.server",
            record.server_name.as_ref().map(McpServerId::as_str),
        )
        .opt_str("systemprompt.mcp.execution_id", execution_id)
        .opt_str("systemprompt.tool.state", record.state.as_deref())
        .opt_str("systemprompt.tool.source", record.source.as_deref())
        .opt_str(
            "systemprompt.tool.status",
            record.execution_status.as_deref(),
        )
        .opt_int("systemprompt.tool.execution_ms", record.execution_time_ms)
        .opt_str(
            "systemprompt.artifact.type",
            record.artifact_type.as_deref(),
        )
        .opt_int("systemprompt.artifact.bytes", record.payload_bytes)
        .opt_int(
            "systemprompt.artifact.secret_redactions",
            record.secret_redactions,
        )
        .flag("systemprompt.artifact.is_error", record.is_error);
    let failed = record.is_error
        || matches!(
            record.execution_status.as_deref(),
            Some("failed" | "timeout")
        );
    Span {
        trace_id: trace_id_bytes(trace_key(record.trace_id.as_ref(), key)),
        span_id: span_id_bytes(TOOL_SPAN, key),
        parent_span_id: parent_of(record.request_id.as_ref()),
        name: format!("{TOOL_SPAN} {}", tool_name.unwrap_or("unknown")),
        kind: SpanKind::Internal as i32,
        start_time_unix_nano: unix_nanos(record.started_at),
        end_time_unix_nano: unix_nanos(record.completed_at.max(record.started_at)),
        attributes: attrs.finish(),
        status: Some(Status {
            code: if failed {
                StatusCode::Error as i32
            } else {
                StatusCode::Ok as i32
            },
            message: record.error_message.clone().unwrap_or_default(),
        }),
        ..Default::default()
    }
}

#[must_use]
pub fn governance_span(record: &GovernanceSpanRecord) -> Span {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.governance.decision", &record.decision)
        .text("systemprompt.governance.policy", &record.policy)
        .text("systemprompt.governance.reason", &record.reason)
        .text("gen_ai.tool.name", &record.tool_name)
        .opt_str("gen_ai.tool.call.id", record.tool_use_id.as_deref())
        .opt_str(
            "systemprompt.plugin.id",
            record.plugin_id.as_ref().map(PluginId::as_str),
        )
        .text("systemprompt.actor.kind", &record.actor_kind)
        .text("systemprompt.actor.id", &record.actor_id);
    let at = unix_nanos(record.decided_at);
    let denied = record.decision == "deny";
    Span {
        trace_id: trace_id_bytes(trace_key(record.trace_id.as_ref(), &record.id)),
        span_id: span_id_bytes(GOVERNANCE_SPAN, &record.id),
        parent_span_id: parent_of(record.request_id.as_ref()),
        name: format!("{GOVERNANCE_SPAN} {}", record.policy),
        kind: SpanKind::Internal as i32,
        start_time_unix_nano: at,
        end_time_unix_nano: at,
        attributes: attrs.finish(),
        status: Some(Status {
            code: if denied {
                StatusCode::Error as i32
            } else {
                StatusCode::Ok as i32
            },
            message: if denied {
                record.reason.clone()
            } else {
                String::new()
            },
        }),
        ..Default::default()
    }
}
