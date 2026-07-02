//! Drives `McpClient::list_tools` / `McpClient::call_tool` end-to-end against
//! a scripted wiremock MCP endpoint through the external-server transport
//! path, including the oauth and external-auth guard branches.

use systemprompt_mcp::services::client::McpClient;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use wiremock::MockServer;

use crate::harness::{
    default_tools_json, external_mcp_config, mount_mcp_endpoint, request_context,
};

#[tokio::test]
async fn list_tools_maps_schemas_and_metadata() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;
    mount_mcp_endpoint(&server, default_tools_json()).await;

    let mut config = external_mcp_config("live_list", &format!("{}/mcp", server.uri()));
    config.tools.insert(
        "echo".to_owned(),
        serde_json::from_value(serde_json::json!({"terminal_on_success": true}))
            .expect("tool metadata"),
    );

    let tools = McpClient::list_tools(&config, &request_context("list"))
        .await
        .expect("tools list");

    assert_eq!(tools.len(), 2);
    let echo = tools.iter().find(|t| t.name == "echo").expect("echo tool");
    assert!(echo.terminal_on_success);
    assert_eq!(echo.description.as_deref(), Some("Echo a message"));
    assert!(echo.input_schema.is_some());
    assert_eq!(echo.service_id.as_str(), "live_list");

    let shout = tools
        .iter()
        .find(|t| t.name == "shout")
        .expect("shout tool");
    assert!(!shout.terminal_on_success);
    assert!(shout.output_schema.is_some());
}

#[tokio::test]
async fn call_tool_round_trips_result() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;
    mount_mcp_endpoint(&server, default_tools_json()).await;

    let config = external_mcp_config("live_call", &format!("{}/mcp", server.uri()));
    let result = McpClient::call_tool(
        &config,
        "echo".to_owned(),
        Some(serde_json::json!({"message": "hi"})),
        &request_context("call"),
    )
    .await
    .expect("tool call");

    assert_eq!(result.is_error, Some(false));
    let text = serde_json::to_string(&result.content).expect("content serializes");
    assert!(text.contains("harness output"));
}

#[tokio::test]
async fn oauth_required_without_token_is_rejected() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;
    mount_mcp_endpoint(&server, default_tools_json()).await;

    let mut config = external_mcp_config("live_auth", &format!("{}/mcp", server.uri()));
    config.oauth.required = true;

    let err = McpClient::list_tools(&config, &request_context("auth"))
        .await
        .expect_err("missing token rejected");
    assert!(err.to_string().contains("User JWT required"));
}

#[tokio::test]
async fn oauth_required_with_token_sends_bearer() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;
    mount_mcp_endpoint(&server, default_tools_json()).await;

    let mut config = external_mcp_config("live_auth_ok", &format!("{}/mcp", server.uri()));
    config.oauth.required = true;

    let context = request_context("auth-ok").with_auth_token("user-jwt".to_owned());
    let tools = McpClient::list_tools(&config, &context)
        .await
        .expect("tools list with bearer");
    assert_eq!(tools.len(), 2);
}

#[tokio::test]
async fn external_auth_without_user_is_rejected() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;

    let mut config = external_mcp_config("live_ext", &format!("{}/mcp", server.uri()));
    config.external_auth = Some(
        serde_yaml::from_str("token_endpoint: /api/public/prov/token").expect("external auth"),
    );

    let err = McpClient::call_tool(&config, "echo".to_owned(), None, &request_context("ext"))
        .await
        .expect_err("anonymous external-auth call rejected");
    assert!(err.to_string().contains("requires an authenticated user"));
}

#[tokio::test]
async fn external_auth_with_unreachable_accessor_fails() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;

    let mut config = external_mcp_config("live_ext2", &format!("{}/mcp", server.uri()));
    config.external_auth = Some(
        serde_yaml::from_str("token_endpoint: /api/public/prov/token").expect("external auth"),
    );

    let context = request_context("ext2").with_auth_token("user-jwt".to_owned());
    let err = McpClient::call_tool(&config, "echo".to_owned(), None, &context)
        .await
        .expect_err("accessor unreachable surfaces");
    assert!(err.to_string().contains("token accessor"));
}
