//! Unit tests for McpClientHandler and HttpClientWithContext constructors.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::services::client::{HttpClientWithContext, rewrite_url_for_internal_use};
use systemprompt_models::RequestContext;

fn sample_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("test"),
    )
}

#[test]
fn http_client_with_context_new_returns_clonable_value() {
    let client = HttpClientWithContext::new(sample_request_context());
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
