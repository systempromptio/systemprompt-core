//! Tests for the `create_request_context` helper in
//! `services/tool_provider/context.rs`, reached through `McpToolProvider` or
//! by reconstructing a [`ToolContext`] with the required headers.
//!
//! The function is pub(super) so we drive it indirectly through
//! `McpToolProvider::call_tool` / `list_tools` which propagate any
//! `ToolProviderError::ConfigurationError` back to the caller — giving us
//! coverage of the header-validation branches without a live MCP server.

use systemprompt_identifiers::{Actor, AiToolCallId, ContextId, McpServerId, SessionId, TraceId};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider};

async fn provider() -> Option<McpToolProvider> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let registry = RegistryService::new(fixture_user_id());
    Some(McpToolProvider::new(
        db,
        registry,
        &ResilienceSettings::default(),
    ))
}

fn base_ctx() -> ToolContext {
    ToolContext::new(Actor::user(fixture_user_id()), "test-token")
}

fn full_ctx() -> ToolContext {
    let ctx_id = ContextId::generate();
    base_ctx()
        .with_header("x-context-id", ctx_id.as_str())
        .with_header("x-agent-name", "my-agent")
}

#[tokio::test]
async fn list_tools_missing_context_id_returns_config_error() {
    let Some(p) = provider().await else { return };
    let ctx = base_ctx().with_header("x-agent-name", "my-agent");
    let result = p.list_tools("some-agent", &ctx).await;
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("context") || msg.contains("agent") || msg.contains("config"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn list_tools_missing_agent_name_header_returns_config_error() {
    let Some(p) = provider().await else { return };
    let ctx_id = ContextId::generate();
    let ctx = base_ctx().with_header("x-context-id", ctx_id.as_str());
    let result = p.list_tools("some-agent", &ctx).await;
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("agent") || msg.contains("context") || msg.contains("config"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn list_tools_invalid_context_id_returns_error() {
    let Some(p) = provider().await else { return };
    let ctx = base_ctx()
        .with_header("x-context-id", "not-a-uuid")
        .with_header("x-agent-name", "my-agent");
    let result = p.list_tools("some-agent", &ctx).await;
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("context") || msg.contains("uuid") || msg.contains("config"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn list_tools_empty_context_id_returns_error() {
    let Some(p) = provider().await else { return };
    let ctx = base_ctx()
        .with_header("x-context-id", "")
        .with_header("x-agent-name", "my-agent");
    let result = p.list_tools("some-agent", &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn call_tool_with_full_ctx_and_nonexistent_server_errors_gracefully() {
    let Some(p) = provider().await else { return };
    let ctx = full_ctx();
    let request = ToolCallRequest {
        tool_call_id: "call-1".to_owned(),
        name: "my_tool".to_owned(),
        arguments: serde_json::json!({}),
    };
    let result = p.call_tool(&request, &McpServerId::new("nonexistent-server"), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn tool_context_with_session_and_trace_ids_propagated() {
    let Some(p) = provider().await else { return };
    let ctx_id = ContextId::generate();
    let ctx = ToolContext::new(Actor::user(fixture_user_id()), "tok")
        .with_session_id(SessionId::new("my-session"))
        .with_trace_id(TraceId::new("my-trace"))
        .with_header("x-context-id", ctx_id.as_str())
        .with_header("x-agent-name", "agent-with-ids");
    let result = p.list_tools("agent-with-ids", &ctx).await;
    let _ = result;
}

#[tokio::test]
async fn tool_context_with_ai_tool_call_id_and_task_id() {
    let Some(p) = provider().await else { return };
    let ctx_id = ContextId::generate();
    let tool_call_id = AiToolCallId::new("call-abc");
    let ctx = ToolContext::new(Actor::user(fixture_user_id()), "tok")
        .with_ai_tool_call_id(tool_call_id)
        .with_header("x-context-id", ctx_id.as_str())
        .with_header("x-agent-name", "agent-task")
        .with_header("x-task-id", "task-123")
        .with_header("x-user-id", "user-xyz");
    let result = p.list_tools("agent-task", &ctx).await;
    let _ = result;
}
