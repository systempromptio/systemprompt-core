//! Unit tests for McpClientHandler and HttpClientWithContext constructors.

use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::services::client::{HttpClientWithContext, rewrite_url_for_internal_use};
use systemprompt_models::RequestContext;

fn sample_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("test").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

#[test]
fn http_client_with_context_new_returns_clonable_value() {
    let client = HttpClientWithContext::new(sample_request_context()).expect("guarded client");
    let cloned = client.clone();
    let _ = format!("{cloned:?}");
}

#[test]
fn rewrite_url_for_internal_use_falls_back_when_config_uninitialised() {
    let url = "http://example.com/mcp";
    let out = rewrite_url_for_internal_use(url);
    assert_eq!(
        out, url,
        "without Config::get, URL passes through unchanged"
    );
}
