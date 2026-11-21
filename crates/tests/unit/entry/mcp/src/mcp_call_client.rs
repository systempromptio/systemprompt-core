//! Tests for the `plugins mcp` client helpers: content conversion, the
//! caller's token on every frame sent to a platform-hosted server,
//! `tools/list` params and the call deadline.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use rmcp::model::ContentBlock;
use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::{ContextId, McpToolName, SessionId, SessionToken, UserId};
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp_entry::cli::call_client::{
    McpTarget, ToolCallParams, convert_content, execute_tool_call, list_tools,
};
use systemprompt_models::auth::UserType;

use crate::mcp_call_credential::server_config;

const CALLER_CONTEXT: &str = "7c1d5e9a-3b2f-4c8d-9e6a-1f0b2c3d4e5f";

fn session() -> CallerSession {
    CallerSession::new(
        SessionToken::new("tok"),
        SessionId::new("sess-caller"),
        UserId::new("user-caller"),
        ContextId::try_new(CALLER_CONTEXT).expect("a v4 context id"),
        UserType::Admin,
    )
}

fn internal(name: &str, port: u16) -> McpServerConfig {
    let mut config = server_config(name);
    config.port = Some(port);
    config
}

fn block(json: serde_json::Value) -> ContentBlock {
    serde_json::from_value(json).unwrap()
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[test]
fn convert_content_maps_text_image_audio_and_resource_link() {
    let text =
        convert_content(&block(serde_json::json!({"type": "text", "text": "hello"}))).unwrap();
    assert_eq!(text.kind, "text");
    assert_eq!(text.text.as_deref(), Some("hello"));

    let image = convert_content(&block(serde_json::json!({
        "type": "image", "data": "aGk=", "mimeType": "image/png"
    })))
    .unwrap();
    assert_eq!(image.kind, "image");
    assert_eq!(image.mime_type.as_deref(), Some("image/png"));
    assert_eq!(image.data.as_deref(), Some("aGk="));

    let audio = convert_content(&block(serde_json::json!({
        "type": "audio", "data": "aGk=", "mimeType": "audio/wav"
    })))
    .unwrap();
    assert_eq!(audio.kind, "audio");
    assert_eq!(audio.mime_type.as_deref(), Some("audio/wav"));

    let link = convert_content(&block(serde_json::json!({
        "type": "resource_link", "uri": "file:///x", "name": "x", "mimeType": "text/plain"
    })))
    .unwrap();
    assert_eq!(link.kind, "resource_link");
    assert_eq!(link.text.as_deref(), Some("file:///x"));
    assert_eq!(link.mime_type.as_deref(), Some("text/plain"));
}

#[test]
fn convert_content_renders_embedded_resources_as_json() {
    let resource = convert_content(&block(serde_json::json!({
        "type": "resource",
        "resource": {"uri": "file:///doc", "text": "body", "mimeType": "text/plain"}
    })))
    .unwrap();
    assert_eq!(resource.kind, "resource");
    let parsed: serde_json::Value = serde_json::from_str(&resource.text.unwrap()).unwrap();
    assert_eq!(parsed["uri"], "file:///doc");
    assert_eq!(parsed["text"], "body");
}

#[tokio::test]
async fn execute_tool_call_fails_fast_against_closed_port() {
    let err = execute_tool_call(ToolCallParams {
        target: &McpTarget::resolve(&internal("svc", free_port()), Some(&session())).unwrap(),
        tool_name: &McpToolName::new("echo"),
        arguments: Some(serde_json::json!({"x": 1})),
        timeout_secs: 5,
    })
    .await
    .unwrap_err();

    assert!(err.to_string().contains("Failed to connect to MCP server"));
}

#[tokio::test]
async fn list_tools_fails_fast_against_closed_port() {
    let err = list_tools(
        &McpTarget::resolve(&internal("svc", free_port()), Some(&session())).unwrap(),
        5,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("Failed to connect to MCP server"));
}

struct JsonRpcMethod(&'static str);

impl wiremock::Match for JsonRpcMethod {
    fn matches(&self, request: &wiremock::Request) -> bool {
        serde_json::from_slice::<serde_json::Value>(&request.body)
            .ok()
            .and_then(|body| {
                body.get("method")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .as_deref()
            == Some(self.0)
    }
}

struct ExactAuthorization(&'static str);

impl wiremock::Match for ExactAuthorization {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let values = request.headers.get_all("authorization");
        let values = values.iter().collect::<Vec<_>>();
        !values.is_empty()
            && values
                .iter()
                .all(|value| value.to_str().ok() == Some(self.0))
    }
}

async fn mount_protocol(server: &wiremock::MockServer) {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, ResponseTemplate};

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "systemprompt-cli-strict",
                    "version": "1.0.0"
                }
            }
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-cli-live")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "strict-fixture", "version": "1.0.0"}
                    }
                })),
        )
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

async fn assert_initialize_frame(server: &wiremock::MockServer) {
    let requests = server
        .received_requests()
        .await
        .expect("recorded MCP requests");
    let initialize_request = requests
        .iter()
        .find(|request| wiremock::Match::matches(&JsonRpcMethod("initialize"), request))
        .expect("recorded initialize request");
    let authorization = initialize_request
        .headers
        .get_all("authorization")
        .iter()
        .map(|value| value.to_str().expect("ASCII authorization value"))
        .collect::<Vec<_>>();
    assert!(
        !authorization.is_empty(),
        "authorization must be propagated"
    );
    assert!(
        authorization.iter().all(|value| *value == "Bearer tok"),
        "every propagated credential must equal the session token: {authorization:?}"
    );
    let initialize: serde_json::Value =
        serde_json::from_slice(&initialize_request.body).expect("initialize JSON-RPC body");
    assert_eq!(
        initialize,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "systemprompt-cli-strict",
                    "version": "1.0.0"
                }
            }
        })
    );
}

#[tokio::test]
async fn list_tools_sends_non_null_object_params_and_preserves_names() {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    mount_protocol(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {"_meta": {"progressToken": 0}}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"tools": [
                        {"name": "alpha", "description": "A", "inputSchema": {"type": "object"}},
                        {"name": "zeta", "description": "Z", "inputSchema": {"type": "object"}}
                    ]}
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let tools_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        list_tools(
            &McpTarget::resolve(
                &internal("strict", server.address().port()),
                Some(&session()),
            )
            .unwrap(),
            5,
        ),
    )
    .await
    .expect("tools/list must finish within ten seconds");
    assert_initialize_frame(&server).await;
    let tools = tools_result.expect("strict tools/list succeeds");
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["alpha", "zeta"]);
    let requests = server
        .received_requests()
        .await
        .expect("recorded MCP requests");
    let list_request = requests
        .iter()
        .map(|request| serde_json::from_slice::<serde_json::Value>(&request.body))
        .find_map(|body| {
            let body = body.ok()?;
            (body["method"] == "tools/list").then_some(body)
        })
        .expect("recorded tools/list request");
    assert_eq!(
        list_request["params"],
        serde_json::json!({"_meta": {"progressToken": 0}})
    );
    assert!(
        list_request["params"].get("cursor").is_none(),
        "the initial list request must not invent a pagination cursor: {list_request}"
    );
}

#[tokio::test]
async fn execute_tool_call_sends_arguments_and_surfaces_jsonrpc_rejection() {
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let success = MockServer::start().await;
    mount_protocol(&success).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call",
            "params": {"name": "echo", "arguments": {"message": "owned-value"}}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "content": [{"type": "text", "text": "owned-result"}],
                        "isError": false
                    }
                })),
        )
        .expect(1)
        .mount(&success)
        .await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        execute_tool_call(ToolCallParams {
            target: &McpTarget::resolve(
                &internal("strict", success.address().port()),
                Some(&session()),
            )
            .unwrap(),
            tool_name: &McpToolName::new("echo"),
            arguments: Some(serde_json::json!({"message": "owned-value"})),
            timeout_secs: 5,
        }),
    )
    .await
    .expect("tools/call must finish within ten seconds");
    assert_initialize_frame(&success).await;
    let result = result.expect("strict tools/call succeeds");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
    assert_eq!(
        convert_content(&result.content[0]).unwrap().text.as_deref(),
        Some("owned-result")
    );

    let rejected = MockServer::start().await;
    mount_protocol(&rejected).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call",
            "params": {"name": "denied"}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": {"code": -32602, "message": "owned rejection"}
                })),
        )
        .expect(1)
        .mount(&rejected)
        .await;
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        execute_tool_call(ToolCallParams {
            target: &McpTarget::resolve(
                &internal("strict", rejected.address().port()),
                Some(&session()),
            )
            .unwrap(),
            tool_name: &McpToolName::new("denied"),
            arguments: None,
            timeout_secs: 5,
        }),
    )
    .await
    .expect("rejected tools/call must finish within ten seconds");
    assert_initialize_frame(&rejected).await;
    let error = error.expect_err("JSON-RPC rejection must fail the call");
    let chain = format!("{error:#}");
    assert!(
        chain.contains("MCP tool 'denied' on 'strict' rejected the call"),
        "{chain}"
    );
    assert!(chain.contains("owned rejection"), "{chain}");
}

#[tokio::test]
async fn execute_tool_call_timeout_bounds_a_call_the_server_never_answers() {
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    mount_protocol(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "hang"}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_delay(std::time::Duration::from_secs(60))
                .set_body_json(serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}})),
        )
        .mount(&server)
        .await;
    let started = std::time::Instant::now();
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        execute_tool_call(ToolCallParams {
            target: &McpTarget::resolve(
                &internal("strict", server.address().port()),
                Some(&session()),
            )
            .unwrap(),
            tool_name: &McpToolName::new("hang"),
            arguments: None,
            timeout_secs: 1,
        }),
    )
    .await
    .expect("the call's own timeout must fire before the test's")
    .expect_err("a call that never answers fails");
    assert!(
        error.to_string().contains("did not finish within 1s"),
        "{error:#}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}
