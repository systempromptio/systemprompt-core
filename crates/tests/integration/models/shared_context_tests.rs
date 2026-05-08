use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::execution::{RequestContext, SharedRequestContext};

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn test_shared_context_creation() {
    let ctx = RequestContext::new(
        SessionId::new("sess-123".to_string()),
        TraceId::new("trace-456".to_string()),
        ContextId::new(TEST_CONTEXT_ID_A),
        AgentName::new("test_agent".to_string()),
    );

    let shared = SharedRequestContext::from(ctx);
    let locked = shared.lock().unwrap();
    assert_eq!(locked.request.session_id.as_str(), "sess-123");
    drop(locked);
}

#[test]
fn test_shared_context_mutation() {
    let ctx = RequestContext::new(
        SessionId::new("sess-123".to_string()),
        TraceId::new("trace-456".to_string()),
        ContextId::new(TEST_CONTEXT_ID_A),
        AgentName::new("test_agent".to_string()),
    );

    let shared = SharedRequestContext::from(ctx);

    {
        let mut locked = shared.lock().unwrap();
        locked.request.session_id = SessionId::new("sess-updated".to_string());
    }

    {
        let locked = shared.lock().unwrap();
        assert_eq!(locked.request.session_id.as_str(), "sess-updated");
        drop(locked);
    }
}

#[test]
fn test_malformed_context_id_try_new_fails() {
    for bad in &["ctx-789", "not-a-uuid", "ctx-1", "12345"] {
        assert!(
            ContextId::try_new(bad.to_string()).is_err(),
            "non-UUID value {bad:?} must not construct a ContextId"
        );
    }
}
