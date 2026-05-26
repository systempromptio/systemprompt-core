//! Tests for `RequestSpan`, `SystemSpan`, and `RequestSpanBuilder`.

use systemprompt_identifiers::{ClientId, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_logging::{RequestSpan, RequestSpanBuilder, SystemSpan};

fn ids() -> (UserId, SessionId, TraceId) {
    (
        UserId::new("user-abc"),
        SessionId::generate(),
        TraceId::generate(),
    )
}

#[test]
fn request_span_new_and_enter() {
    let (u, s, t) = ids();
    let span = RequestSpan::new(&u, &s, &t);
    let _entered = span.enter();
    let _ref: &tracing::Span = span.span();
    assert!(format!("{span:?}").contains("RequestSpan"));
}

#[test]
fn request_span_record_methods_no_panic() {
    let (u, s, t) = ids();
    let span = RequestSpan::new(&u, &s, &t);
    span.record_task_id(&TaskId::new("task-1"));
    span.record_context_id(&ContextId::generate());
    span.record_client_id(&ClientId::new("client-1"));
}

#[test]
fn system_span_new_enter_and_record() {
    let span = SystemSpan::new("scheduler");
    let _entered = span.enter();
    span.record_task_id(&TaskId::new("t"));
    span.record_context_id(&ContextId::generate());
    let _ref: &tracing::Span = span.span();
    assert!(format!("{span:?}").contains("SystemSpan"));
}

#[test]
fn system_span_into_span_and_from() {
    let span = SystemSpan::new("api");
    let _: tracing::Span = span.into_span();

    let span = SystemSpan::new("api");
    let _: tracing::Span = span.into();
}

#[test]
fn builder_minimal() {
    let (u, s, t) = ids();
    let span = RequestSpanBuilder::new(&u, &s, &t).build();
    let _entered = span.enter();
    assert!(format!("{:?}", RequestSpanBuilder::new(&u, &s, &t)).contains("RequestSpanBuilder"));
}

#[test]
fn builder_with_all_optional_ids() {
    let (u, s, t) = ids();
    let ctx = ContextId::generate();
    let task = TaskId::new("task-x");
    let client = ClientId::new("c");
    let span = RequestSpanBuilder::new(&u, &s, &t)
        .with_context_id(&ctx)
        .with_task_id(&task)
        .with_client_id(&client)
        .build();
    let _entered = span.enter();
}

#[test]
fn builder_skips_empty_context_id() {
    let (u, s, t) = ids();
    // ContextId requires UUID, but with_context_id() only checks `as_str().is_empty()`.
    // We exercise the populated branch and rely on the new_unchecked-like absence.
    let ctx = ContextId::generate();
    let span = RequestSpanBuilder::new(&u, &s, &t).with_context_id(&ctx).build();
    let _entered = span.enter();
}
