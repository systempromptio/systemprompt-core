//! Audit rows → OTLP spans.
//!
//! One span per completed `ai_requests` row, with a child span for every
//! tool call in the ledger that belongs to it and every governance decision
//! made under its trace. Pure: no I/O, so the shape is unit-testable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::status::StatusCode;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status};

use super::attrs::{Attrs, resource, scope};
use super::ids::{span_id_bytes, trace_id_bytes, unix_nanos};
use super::tail::{GovernanceRow, LedgerRow, RequestRow};
use systemprompt_identifiers::{SessionId, TraceId};

pub const REQUEST_SPAN: &str = "ai_request";
pub const TOOL_SPAN: &str = "tool_call";
pub const GOVERNANCE_SPAN: &str = "governance_decision";

/// A batch of requests with the child rows already fetched for them.
#[derive(Debug, Default)]
pub struct TraceBatch {
    pub requests: Vec<RequestRow>,
    pub ledger: Vec<LedgerRow>,
    pub governance: Vec<GovernanceRow>,
}

impl TraceBatch {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    #[must_use]
    pub fn trace_ids(&self) -> Vec<String> {
        self.requests
            .iter()
            .filter_map(|r| r.trace_id.as_ref())
            .map(TraceId::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

fn trace_key(request: &RequestRow) -> &str {
    request
        .trace_id
        .as_ref()
        .map(TraceId::as_str)
        .filter(|t| !t.is_empty())
        .unwrap_or(request.id.as_str())
}

#[must_use]
pub(super) fn to_export_request(
    batch: &TraceBatch,
    instance_id: Option<&str>,
) -> ExportTraceServiceRequest {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(resource(instance_id)),
            scope_spans: vec![ScopeSpans {
                scope: Some(scope()),
                spans: to_spans(batch),
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    }
}

#[must_use]
pub fn to_spans(batch: &TraceBatch) -> Vec<Span> {
    let mut by_request: HashMap<&str, Vec<&LedgerRow>> = HashMap::new();
    for row in &batch.ledger {
        if let Some(request_id) = row.request_id.as_deref() {
            by_request.entry(request_id).or_default().push(row);
        }
    }
    let mut by_trace: HashMap<&str, Vec<&GovernanceRow>> = HashMap::new();
    for row in &batch.governance {
        if let Some(trace_id) = row.trace_id.as_ref() {
            by_trace.entry(trace_id.as_str()).or_default().push(row);
        }
    }

    let mut spans = Vec::with_capacity(batch.requests.len());
    for request in &batch.requests {
        let trace = trace_id_bytes(trace_key(request));
        let parent = span_id_bytes(REQUEST_SPAN, &request.id);
        spans.push(request_span(request, trace.clone(), parent.clone()));
        for row in by_request.get(request.id.as_str()).into_iter().flatten() {
            spans.push(tool_span(row, trace.clone(), parent.clone()));
        }
        // Why: decisions key on the gateway trace id, not the request row, so
        // a request that carries no trace id has no decisions to attach.
        let Some(trace_id) = request.trace_id.as_ref() else {
            continue;
        };
        for row in by_trace.get(trace_id.as_str()).into_iter().flatten() {
            spans.push(governance_span(row, trace.clone(), parent.clone()));
        }
    }
    spans
}

fn request_span(row: &RequestRow, trace_id: Vec<u8>, span_id: Vec<u8>) -> Span {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.request.id", &row.request_id)
        .text("systemprompt.request.kind", &row.request_kind)
        .text("systemprompt.request.status", &row.status)
        .text("enduser.id", row.user_id.as_str())
        .text("systemprompt.actor.kind", &row.actor_kind)
        .text("systemprompt.actor.id", &row.actor_id)
        .opt_str("session.id", row.session_id.as_ref().map(SessionId::as_str))
        .text("systemprompt.context.id", row.context_id.as_str())
        .opt_str(
            "systemprompt.trace.id",
            row.trace_id.as_ref().map(TraceId::as_str),
        )
        .opt_str("gen_ai.system", row.provider.as_deref())
        .opt_str(
            "systemprompt.served_provider",
            row.served_provider.as_deref(),
        )
        .opt_str("gen_ai.request.model", row.requested_model.as_deref())
        .opt_str("gen_ai.response.model", row.model.as_deref())
        .opt_str("systemprompt.route", row.route_match.as_deref())
        .opt_int("gen_ai.usage.input_tokens", row.input_tokens)
        .opt_int("gen_ai.usage.output_tokens", row.output_tokens)
        .opt_int("gen_ai.usage.cache_read_tokens", row.cache_read_tokens)
        .opt_int(
            "gen_ai.usage.cache_creation_tokens",
            row.cache_creation_tokens,
        )
        .float(
            "systemprompt.cost.usd",
            row.cost_microdollars as f64 / 1_000_000.0,
        )
        .int("systemprompt.cost.microdollars", row.cost_microdollars)
        .opt_int("systemprompt.latency_ms", row.latency_ms)
        .opt_int("systemprompt.upstream_latency_ms", row.upstream_latency_ms)
        .opt_str(
            "gen_ai.response.finish_reason",
            row.finish_reason.as_deref(),
        )
        .text("systemprompt.client.kind", &row.client_kind)
        .text("systemprompt.wire_protocol", &row.wire_protocol)
        .opt_str("systemprompt.instance.id", row.instance_id.as_deref());
    let (code, message) = request_status(row);
    Span {
        trace_id,
        span_id,
        name: format!(
            "{REQUEST_SPAN} {}",
            row.model.as_deref().unwrap_or("unrouted")
        ),
        kind: SpanKind::Server as i32,
        start_time_unix_nano: unix_nanos(row.created_at),
        end_time_unix_nano: unix_nanos(row.completed_at),
        attributes: attrs.finish(),
        status: Some(Status {
            code: code as i32,
            message,
        }),
        ..Default::default()
    }
}

fn request_status(row: &RequestRow) -> (StatusCode, String) {
    match row.status.as_str() {
        "completed" => (StatusCode::Ok, String::new()),
        "pending" => (StatusCode::Unset, String::new()),
        _ => (
            StatusCode::Error,
            row.error_message
                .clone()
                .unwrap_or_else(|| row.status.clone()),
        ),
    }
}

fn tool_span(row: &LedgerRow, trace_id: Vec<u8>, parent_span_id: Vec<u8>) -> Span {
    let key = row
        .ai_tool_call_id
        .as_deref()
        .or(row.mcp_execution_id.as_deref())
        .unwrap_or_default();
    let (start, end) = tool_window(row);
    let mut attrs = Attrs::new();
    attrs
        .opt_str("gen_ai.tool.name", row.tool_name.as_deref())
        .opt_str("gen_ai.tool.call.id", row.ai_tool_call_id.as_deref())
        .opt_str("systemprompt.mcp.server", row.server_name.as_deref())
        .opt_str(
            "systemprompt.mcp.execution_id",
            row.mcp_execution_id.as_deref(),
        )
        .opt_str("systemprompt.tool.state", row.state.as_deref())
        .opt_str("systemprompt.tool.source", row.source.as_deref())
        .opt_str("systemprompt.tool.status", row.execution_status.as_deref())
        .opt_int("systemprompt.tool.execution_ms", row.execution_time_ms)
        .opt_str("systemprompt.artifact.type", row.artifact_type.as_deref())
        .opt_int("systemprompt.artifact.bytes", row.payload_bytes)
        .opt_int(
            "systemprompt.artifact.secret_redactions",
            row.secret_redactions,
        )
        .flag(
            "systemprompt.artifact.is_error",
            row.is_error.unwrap_or(false),
        );
    let failed = row.is_error.unwrap_or(false)
        || matches!(row.execution_status.as_deref(), Some("failed" | "timeout"));
    Span {
        trace_id,
        span_id: span_id_bytes(TOOL_SPAN, key),
        parent_span_id,
        name: format!(
            "{TOOL_SPAN} {}",
            row.tool_name.as_deref().unwrap_or("unknown")
        ),
        kind: SpanKind::Internal as i32,
        start_time_unix_nano: unix_nanos(start),
        end_time_unix_nano: unix_nanos(end),
        attributes: attrs.finish(),
        status: Some(Status {
            code: if failed {
                StatusCode::Error as i32
            } else {
                StatusCode::Ok as i32
            },
            message: row.error_message.clone().unwrap_or_default(),
        }),
        ..Default::default()
    }
}

fn tool_window(row: &LedgerRow) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = row
        .executed_at
        .or(row.occurred_at)
        .or(row.intended_at)
        .unwrap_or_default();
    let end = row.completed_at.unwrap_or_else(|| {
        start + Duration::milliseconds(i64::from(row.execution_time_ms.unwrap_or(0)))
    });
    (start, end.max(start))
}

fn governance_span(row: &GovernanceRow, trace_id: Vec<u8>, parent_span_id: Vec<u8>) -> Span {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.governance.decision", &row.decision)
        .text("systemprompt.governance.policy", &row.policy)
        .text("systemprompt.governance.reason", &row.reason)
        .text("gen_ai.tool.name", &row.tool_name)
        .opt_str("gen_ai.tool.call.id", row.tool_use_id.as_deref())
        .opt_str("systemprompt.plugin.id", row.plugin_id.as_deref())
        .text("systemprompt.actor.kind", &row.actor_kind)
        .text("systemprompt.actor.id", &row.actor_id);
    let at = unix_nanos(row.created_at);
    Span {
        trace_id,
        span_id: span_id_bytes(GOVERNANCE_SPAN, &row.id),
        parent_span_id,
        name: format!("{GOVERNANCE_SPAN} {}", row.policy),
        kind: SpanKind::Internal as i32,
        start_time_unix_nano: at,
        end_time_unix_nano: at,
        attributes: attrs.finish(),
        status: Some(Status {
            code: if row.decision == "deny" {
                StatusCode::Error as i32
            } else {
                StatusCode::Ok as i32
            },
            message: if row.decision == "deny" {
                row.reason.clone()
            } else {
                String::new()
            },
        }),
        ..Default::default()
    }
}
