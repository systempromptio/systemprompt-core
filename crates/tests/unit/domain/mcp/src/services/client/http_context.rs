//! Wiremock-backed tests for [`HttpClientWithContext`], covering context/header
//! propagation and the GET/POST/DELETE branches of the streamable-HTTP
//! transport.

use futures::StreamExt;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::streamable_http_client::{StreamableHttpClient, StreamableHttpPostResponse};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::services::client::HttpClientWithContext;
use systemprompt_models::RequestContext;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-http"),
        TraceId::new("t-http"),
        ContextId::generate(),
        AgentName::new("agent-http"),
    )
    .with_actor(Actor::user(UserId::new("user-http")))
    .with_auth_token("jwt-token")
}

fn ping() -> ClientJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ping"
    }))
    .expect("valid jsonrpc request")
}

fn uri(server: &MockServer) -> Arc<str> {
    Arc::from(format!("{}/mcp", server.uri()))
}

#[tokio::test]
async fn post_message_json_response_returns_json_variant() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-42")
                .set_body_raw(
                    r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
                    "application/json",
                ),
        )
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let response = client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect("json response");

    match response {
        StreamableHttpPostResponse::Json(_, session) => {
            assert_eq!(session.as_deref(), Some("sess-42"));
        },
        other => panic!("expected Json variant, got {other:?}"),
    }
}

#[tokio::test]
async fn post_message_accepted_returns_accepted_variant() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let response = client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect("accepted response");

    assert!(matches!(response, StreamableHttpPostResponse::Accepted));
}

#[tokio::test]
async fn post_message_sse_response_returns_stream() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "sess-sse")
                .set_body_raw(
                    "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n",
                    "text/event-stream",
                ),
        )
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let response = client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect("sse response");

    match response {
        StreamableHttpPostResponse::Sse(mut stream, session) => {
            assert_eq!(session.as_deref(), Some("sess-sse"));
            let event = stream.next().await.expect("one event").expect("valid sse");
            assert!(event.data.unwrap_or_default().contains("jsonrpc"));
        },
        other => panic!("expected Sse variant, got {other:?}"),
    }
}

#[tokio::test]
async fn post_message_unauthorized_surfaces_www_authenticate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(401).insert_header("www-authenticate", "Bearer realm=\"mcp\""),
        )
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let err = client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect_err("401 must error");

    assert!(err.to_string().contains("auth required"));
}

#[tokio::test]
async fn post_message_unexpected_content_type_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("plain", "text/plain"))
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let err = client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect_err("text/plain must error");

    assert!(err.to_string().to_lowercase().contains("content type"));
}

#[tokio::test]
async fn get_stream_method_not_allowed_maps_to_unsupported_sse() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let err = match client
        .get_stream(uri(&server), Arc::from("sess"), None, None, HashMap::new())
        .await
    {
        Ok(_) => panic!("405 must map to unsupported"),
        Err(e) => e,
    };

    assert!(err.to_string().to_lowercase().contains("support"));
}

#[tokio::test]
async fn get_stream_success_yields_sse_events() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("data: hello\n\n", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let mut stream = client
        .get_stream(
            uri(&server),
            Arc::from("sess"),
            Some("evt-1".to_owned()),
            None,
            HashMap::new(),
        )
        .await
        .expect("stream opens");

    let event = stream.next().await.expect("one event").expect("valid sse");
    assert_eq!(event.data.as_deref(), Some("hello"));
}

#[tokio::test]
async fn get_stream_wrong_content_type_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let err = match client
        .get_stream(uri(&server), Arc::from("sess"), None, None, HashMap::new())
        .await
    {
        Ok(_) => panic!("json content type must error"),
        Err(e) => e,
    };

    assert!(err.to_string().to_lowercase().contains("content type"));
}

#[tokio::test]
async fn delete_session_tolerates_method_not_allowed() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    client
        .delete_session(uri(&server), Arc::from("sess"), None, HashMap::new())
        .await
        .expect("405 is tolerated");
}

#[tokio::test]
async fn delete_session_success_and_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    client
        .delete_session(uri(&server), Arc::from("sess"), None, HashMap::new())
        .await
        .expect("200 deletes");

    let failing = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&failing)
        .await;

    let err = client
        .delete_session(uri(&failing), Arc::from("sess"), None, HashMap::new())
        .await
        .expect_err("500 errors");
    assert!(err.to_string().contains("500"));
}

#[tokio::test]
async fn forwarding_client_sends_context_and_bearer_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    let mut outbound = HashMap::new();
    outbound.insert(
        http::HeaderName::from_static("x-static-extra"),
        http::HeaderValue::from_static("extra"),
    );
    let client = HttpClientWithContext::forwarding(ctx(), outbound);
    client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect("accepted");

    let requests = server.received_requests().await.expect("recorded");
    let headers = &requests[0].headers;
    assert_eq!(
        headers.get("x-session-id").and_then(|v| v.to_str().ok()),
        Some("s-http")
    );
    assert_eq!(
        headers.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer jwt-token")
    );
    assert_eq!(
        headers.get("x-static-extra").and_then(|v| v.to_str().ok()),
        Some("extra")
    );
}

#[tokio::test]
async fn external_client_withholds_context_and_internal_bearer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    let mut outbound = HashMap::new();
    outbound.insert(
        http::HeaderName::from_static("authorization"),
        http::HeaderValue::from_static("Bearer third-party"),
    );
    let client = HttpClientWithContext::external(ctx(), outbound);
    client
        .post_message(uri(&server), ping(), None, None, HashMap::new())
        .await
        .expect("accepted");

    let requests = server.received_requests().await.expect("recorded");
    let headers = &requests[0].headers;
    assert!(headers.get("x-session-id").is_none());
    assert!(headers.get("x-trace-id").is_none());
    assert_eq!(
        headers.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer third-party")
    );
}
