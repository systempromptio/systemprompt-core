//! The platform session reaches only the API: `plugins mcp tools|call` connect
//! to `<api_external_url>/api/v1/mcp/<server>/mcp` with the caller's own
//! session and ids, and a missing API is a typed `ApiUnreachable`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::{ContextId, McpServerId, SessionId, SessionToken, UserId};
use systemprompt_mcp_entry::cli::probe::{McpCallError, McpTarget, list_tools, proxy_url};
use systemprompt_models::auth::UserType;

const TOKEN: &str = "platform-session-token";

const CALLER_CONTEXT: &str = "7c1d5e9a-3b2f-4c8d-9e6a-1f0b2c3d4e5f";

fn caller() -> CallerSession {
    CallerSession::new(
        SessionToken::new(TOKEN),
        SessionId::new("sess-caller"),
        UserId::new("user-caller"),
        ContextId::try_new(CALLER_CONTEXT).expect("a v4 context id"),
        UserType::Admin,
    )
}

#[test]
fn the_target_is_the_api_proxy_route_for_the_server() {
    let server = McpServerId::new("hosted");
    assert_eq!(
        proxy_url("https://api.example.test/", &server),
        "https://api.example.test/api/v1/mcp/hosted/mcp"
    );
    let caller = caller();
    let target = McpTarget::through_proxy("http://127.0.0.1:8080", server, &caller);
    assert_eq!(target.url(), "http://127.0.0.1:8080/api/v1/mcp/hosted/mcp");
}

#[test]
fn the_caller_session_attributes_a_request_to_the_caller() {
    let context = caller().request_context(systemprompt_identifiers::AgentName::system());
    assert_eq!(context.user_id().as_str(), "user-caller");
    assert_eq!(context.context_id().as_str(), CALLER_CONTEXT);
    assert_eq!(context.session_id().as_str(), "sess-caller");
    assert_eq!(
        context.auth_token().map(|token| token.as_str()),
        Some(TOKEN)
    );
}

#[tokio::test]
async fn the_api_proxy_receives_the_callers_token_and_ids() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let api = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/mcp/hosted/mcp"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1..)
        .mount(&api)
        .await;

    let caller = caller();
    let target = McpTarget::through_proxy(&api.uri(), McpServerId::new("hosted"), &caller);
    let _refused_by_fixture = list_tools(&target, 5).await;

    let requests = api.received_requests().await.expect("recorded MCP requests");
    let first = requests.first().expect("the client reached the API");
    assert_eq!(first.url.path(), "/api/v1/mcp/hosted/mcp");
    let headers: Vec<String> = first
        .headers
        .iter()
        .map(|(_, value)| value.to_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        headers.iter().any(|value| value == &format!("Bearer {TOKEN}")),
        "the caller's token is presented to the API: {headers:?}"
    );
    for id in ["sess-caller", CALLER_CONTEXT] {
        assert!(
            headers.iter().any(|value| value == id),
            "the request carries the caller's own {id}: {headers:?}"
        );
    }
}

#[tokio::test]
async fn no_api_answering_is_api_unreachable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let caller = caller();
    let target = McpTarget::through_proxy(
        &format!("http://127.0.0.1:{port}"),
        McpServerId::new("hosted"),
        &caller,
    );
    let error = list_tools(&target, 5).await.unwrap_err();
    assert!(
        matches!(&error, McpCallError::ApiUnreachable { url, .. } if url.ends_with("/api/v1/mcp/hosted/mcp")),
        "{error:#}"
    );
}
