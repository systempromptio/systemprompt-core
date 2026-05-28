//! Tests for `create_request_span`.
//!
//! The function builds a `RequestSpan` from a `RequestContext` and applies
//! optional fields (context_id, task_id, client_id) based on the source data.
//! Each branch is exercised by toggling those optional inputs.

use systemprompt_identifiers::{AgentName, ClientId, ContextId, SessionId, TaskId, TraceId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::create_request_span;

fn base_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("session-1"),
        TraceId::new("trace-1"),
        ContextId::generate(),
        AgentName::new("agent-1"),
    )
}

#[test]
fn span_builds_with_context_id() {
    let ctx = base_ctx();
    let _span = create_request_span(&ctx);
}

#[test]
fn span_builds_with_task_id() {
    let ctx = base_ctx().with_task_id(TaskId::generate());
    let _span = create_request_span(&ctx);
}

#[test]
fn span_builds_with_client_id() {
    let ctx = base_ctx().with_client_id(ClientId::new("client-1"));
    let _span = create_request_span(&ctx);
}

#[test]
fn span_builds_with_all_optional_fields() {
    let ctx = base_ctx()
        .with_task_id(TaskId::generate())
        .with_client_id(ClientId::new("client-2"));
    let _span = create_request_span(&ctx);
}
