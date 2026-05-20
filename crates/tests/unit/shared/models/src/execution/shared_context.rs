//! Unit tests for [`systemprompt_models::execution::SharedRequestContext`].

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::execution::{RequestContext, SharedRequestContext};

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-123"),
        TraceId::new("trace-456"),
        ContextId::new(TEST_CONTEXT_ID_A),
        AgentName::new("test_agent"),
    )
}

#[test]
fn shared_context_creation() {
    let shared = SharedRequestContext::from(ctx());
    let locked = shared.lock().unwrap();
    assert_eq!(locked.request.session_id.as_str(), "sess-123");
}

#[test]
fn shared_context_mutation() {
    let shared = SharedRequestContext::from(ctx());

    {
        let mut locked = shared.lock().unwrap();
        locked.request.session_id = SessionId::new("sess-updated");
    }

    let locked = shared.lock().unwrap();
    assert_eq!(locked.request.session_id.as_str(), "sess-updated");
}

#[test]
fn malformed_context_id_try_new_fails() {
    for bad in &["ctx-789", "not-a-uuid", "ctx-1", "12345"] {
        assert!(
            ContextId::try_new(bad.to_string()).is_err(),
            "non-UUID value {bad:?} must not construct a ContextId"
        );
    }
}
