//! Drives `execute_tool_call` end-to-end against a scripted wiremock MCP
//! endpoint: initialize handshake, initialized notification, tools/call, and
//! session teardown.

use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::collections::HashMap;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::services::client::{HttpClientWithContext, execute_tool_call};
use systemprompt_models::RequestContext;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-exec"),
        TraceId::new("t-exec"),
        ContextId::generate(),
        AgentName::new("agent-exec"),
    )
    .with_actor(Actor::user(UserId::new("user-exec")))
}

async fn mount_mcp_server(server: &MockServer, tool_response: serde_json::Value) {
    let initialize_result = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "scripted", "version": "1.0.0"}
        }
    });

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "initialize"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-exec")
                .set_body_json(initialize_result),
        )
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(tool_response),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

fn transport(server: &MockServer) -> StreamableHttpClientTransport<HttpClientWithContext> {
    let client = HttpClientWithContext::forwarding(ctx(), HashMap::new());
    let config = StreamableHttpClientTransportConfig::with_uri(format!("{}/mcp", server.uri()));
    StreamableHttpClientTransport::with_client(client, config)
}

#[tokio::test]
async fn execute_tool_call_returns_tool_result() {
    let server = MockServer::start().await;
    mount_mcp_server(
        &server,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{"type": "text", "text": "tool output"}],
                "isError": false
            }
        }),
    )
    .await;

    let result = execute_tool_call(
        transport(&server),
        "scripted",
        "echo",
        Some(serde_json::json!({"message": "hi"})),
    )
    .await
    .expect("tool call succeeds");

    assert_eq!(result.is_error, Some(false));
    let text = serde_json::to_string(&result.content).expect("serializable content");
    assert!(text.contains("tool output"));
}

#[tokio::test]
async fn execute_tool_call_surfaces_jsonrpc_error() {
    let server = MockServer::start().await;
    mount_mcp_server(
        &server,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32602, "message": "unknown tool"}
        }),
    )
    .await;

    let err = execute_tool_call(transport(&server), "scripted", "missing", None)
        .await
        .expect_err("jsonrpc error surfaces");

    assert!(err.to_string().contains("unknown tool"));
}
