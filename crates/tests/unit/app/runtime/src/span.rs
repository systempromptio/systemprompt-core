//! Unit tests for request span creation
//!
//! Tests cover:
//! - create_request_span with minimal context
//! - create_request_span with context_id
//! - create_request_span with task_id
//! - create_request_span with client_id
//! - create_request_span with fully populated context

use systemprompt_identifiers::{AgentName, ClientId, ContextId, SessionId, TaskId, TraceId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::create_request_span;

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("test-session"),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    )
}

// ============================================================================
// Basic Span Creation Tests
// ============================================================================

#[test]
fn test_create_request_span_basic() {
    let ctx = create_test_request_context();
    let span = create_request_span(&ctx);
    let _guard = span.enter();
    // Span should be created and enterable
}

#[test]
fn test_create_request_span_with_empty_context_id() {
    let ctx = RequestContext::new(
        SessionId::new("session-123"),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    );
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_new_trace() {
    let ctx = RequestContext::new(
        SessionId::new("new-trace-session"),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    );
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Span Creation with Context ID Tests
// ============================================================================

#[test]
fn test_create_request_span_with_context_id() {
    let ctx = create_test_request_context().with_context_id(ContextId::generate());
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_specific_context_id() {
    let ctx =
        create_test_request_context().with_context_id(ContextId::new("specific-context-123"));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_generated_context_id() {
    let ctx = create_test_request_context().with_context_id(ContextId::generate());
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Span Creation with Task ID Tests
// ============================================================================

#[test]
fn test_create_request_span_with_task_id() {
    let ctx = create_test_request_context().with_task_id(TaskId::generate());
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_specific_task_id() {
    let ctx = create_test_request_context().with_task_id(TaskId::new("task-abc-123"));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Span Creation with Client ID Tests
// ============================================================================

#[test]
fn test_create_request_span_with_client_id() {
    let mut ctx = create_test_request_context();
    ctx.request.client_id = Some(ClientId::new("test-client".to_string()));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_specific_client_id() {
    let mut ctx = create_test_request_context();
    ctx.request.client_id = Some(ClientId::new("client-xyz-789".to_string()));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_empty_client_id() {
    let mut ctx = create_test_request_context();
    ctx.request.client_id = Some(ClientId::new("".to_string()));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Fully Populated Context Tests
// ============================================================================

#[test]
fn test_create_request_span_fully_populated() {
    let mut ctx = create_test_request_context()
        .with_context_id(ContextId::generate())
        .with_task_id(TaskId::generate());
    ctx.request.client_id = Some(ClientId::new("full-client".to_string()));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_all_specific_ids() {
    let mut ctx = RequestContext::new(
        SessionId::new("specific-session"),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    )
    .with_context_id(ContextId::new("specific-context"))
    .with_task_id(TaskId::new("specific-task"));
    ctx.request.client_id = Some(ClientId::new("specific-client".to_string()));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Multiple Span Tests
// ============================================================================

#[test]
fn test_create_multiple_request_spans() {
    let ctx1 = create_test_request_context();
    let ctx2 = create_test_request_context().with_context_id(ContextId::generate());
    let ctx3 = create_test_request_context().with_task_id(TaskId::generate());

    let span1 = create_request_span(&ctx1);
    let span2 = create_request_span(&ctx2);
    let span3 = create_request_span(&ctx3);

    let _guard1 = span1.enter();
    let _guard2 = span2.enter();
    let _guard3 = span3.enter();
}

#[test]
fn test_create_request_span_sequential() {
    for i in 0..5 {
        let ctx = RequestContext::new(
            SessionId::new(format!("session-{}", i)),
            TraceId::generate(),
            ContextId::new(""),
            AgentName::system(),
        );
        let span = create_request_span(&ctx);
        let _guard = span.enter();
    }
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[test]
fn test_create_request_span_with_long_session_id() {
    let long_session = "s".repeat(256);
    let ctx = RequestContext::new(
        SessionId::new(&long_session),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    );
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_special_chars_session() {
    let ctx = RequestContext::new(
        SessionId::new("session-with_special.chars:v1"),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    );
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

#[test]
fn test_create_request_span_with_unicode_client() {
    let mut ctx = create_test_request_context();
    ctx.request.client_id = Some(ClientId::new("クライアント".to_string()));
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Span Reuse Tests
// ============================================================================

#[test]
fn test_create_request_span_same_context_multiple_times() {
    let ctx = create_test_request_context();

    let span1 = create_request_span(&ctx);
    let span2 = create_request_span(&ctx);

    let _guard1 = span1.enter();
    let _guard2 = span2.enter();
}

#[test]
fn test_create_request_span_context_with_all_optional_none() {
    let ctx = RequestContext::new(
        SessionId::new("minimal-session"),
        TraceId::generate(),
        ContextId::new(""),
        AgentName::system(),
    );
    // No context_id, no task_id, no client_id
    let span = create_request_span(&ctx);
    let _guard = span.enter();
}

// ============================================================================
// Context Modification Tests
// ============================================================================

#[test]
fn test_create_request_span_after_context_id_update() {
    let ctx = create_test_request_context();
    let span1 = create_request_span(&ctx);
    let _guard1 = span1.enter();

    let ctx = ctx.with_context_id(ContextId::generate());
    let span2 = create_request_span(&ctx);
    let _guard2 = span2.enter();
}

#[test]
fn test_create_request_span_after_task_id_update() {
    let ctx = create_test_request_context();
    let span1 = create_request_span(&ctx);
    let _guard1 = span1.enter();

    let ctx = ctx.with_task_id(TaskId::generate());
    let span2 = create_request_span(&ctx);
    let _guard2 = span2.enter();
}
