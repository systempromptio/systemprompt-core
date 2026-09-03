//! Tests for `SystemSpan`.

use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_logging::SystemSpan;

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
