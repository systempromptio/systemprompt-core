//! Child spans of a request span: ledger tool calls and governance decisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::status::StatusCode;
use opentelemetry_proto::tonic::trace::v1::{Span, Status};

use super::attrs::Attrs;
use super::ids::{span_id_bytes, unix_nanos};
use super::spans::{GOVERNANCE_SPAN, TOOL_SPAN};
use crate::repository::otlp::{GovernanceRow, LedgerRow};
use systemprompt_identifiers::{AiToolCallId, McpExecutionId, McpServerId, McpToolName, PluginId};

pub(super) fn tool_span(row: &LedgerRow, trace_id: Vec<u8>, parent_span_id: Vec<u8>) -> Span {
    let tool_name = row.tool_name.as_ref().map(McpToolName::as_str);
    let call_id = row.ai_tool_call_id.as_ref().map(AiToolCallId::as_str);
    let execution_id = row.mcp_execution_id.as_ref().map(McpExecutionId::as_str);
    let key = call_id.or(execution_id).unwrap_or_default();
    let (start, end) = tool_window(row);
    let mut attrs = Attrs::new();
    attrs
        .opt_str("gen_ai.tool.name", tool_name)
        .opt_str("gen_ai.tool.call.id", call_id)
        .opt_str(
            "systemprompt.mcp.server",
            row.server_name.as_ref().map(McpServerId::as_str),
        )
        .opt_str("systemprompt.mcp.execution_id", execution_id)
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
        name: format!("{TOOL_SPAN} {}", tool_name.unwrap_or("unknown")),
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

pub(super) fn governance_span(
    row: &GovernanceRow,
    trace_id: Vec<u8>,
    parent_span_id: Vec<u8>,
) -> Span {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.governance.decision", &row.decision)
        .text("systemprompt.governance.policy", &row.policy)
        .text("systemprompt.governance.reason", &row.reason)
        .text("gen_ai.tool.name", &row.tool_name)
        .opt_str("gen_ai.tool.call.id", row.tool_use_id.as_deref())
        .opt_str(
            "systemprompt.plugin.id",
            row.plugin_id.as_ref().map(PluginId::as_str),
        )
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
