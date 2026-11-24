//! The `tool_call` audit span of a recorded execution, handed to the injected
//! [`AuditSpanSink`] when the ledger write that records the execution's end
//! succeeds. The request that emitted the call is not known here, so the span
//! is parented by trace only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{McpExecutionId, McpServerId, McpToolName};
use systemprompt_models::RequestContext;
use systemprompt_models::audit_spans::{AuditSpanSink, ToolCallSpanRecord};
use systemprompt_models::mcp::ExecutionSource;

use crate::models::ToolExecutionResult;

/// The call an execution's completion belongs to.
#[derive(Debug, Clone, Copy)]
pub struct ToolCallSubject<'a> {
    pub tool_name: &'a McpToolName,
    pub server_name: &'a McpServerId,
    pub context: &'a RequestContext,
    pub source: ExecutionSource,
}

pub(super) fn emit(
    sink: &dyn AuditSpanSink,
    mcp_execution_id: &McpExecutionId,
    subject: ToolCallSubject<'_>,
    result: &ToolExecutionResult,
) {
    let completed_at = result.completed_at.unwrap_or_else(Utc::now);
    let ai_tool_call_id = subject.context.ai_tool_call_id().cloned();
    let trace = subject.context.trace_id();
    sink.tool_call(ToolCallSpanRecord {
        state: Some(
            if ai_tool_call_id.is_some() {
                "executed"
            } else {
                "unattested"
            }
            .to_owned(),
        ),
        ai_tool_call_id,
        request_id: None,
        trace_id: (!trace.as_str().is_empty()).then(|| trace.clone()),
        mcp_execution_id: Some(mcp_execution_id.clone()),
        tool_name: Some(subject.tool_name.clone()),
        server_name: Some(subject.server_name.clone()),
        source: Some(subject.source.as_str().to_owned()),
        execution_status: Some(result.status.as_str().to_owned()),
        execution_time_ms: result
            .completed_at
            .map(|done| (done - result.started_at).num_milliseconds() as i32),
        error_message: result.error_message.clone(),
        artifact_type: None,
        payload_bytes: None,
        secret_redactions: None,
        is_error: result.error_message.is_some(),
        started_at: result.started_at,
        completed_at,
    });
}
