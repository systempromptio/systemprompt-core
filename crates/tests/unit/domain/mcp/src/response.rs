//! Unit tests for `McpResponseBuilder` static helpers and constructor.
//! Full `build()` exercises the artifact repository, which requires a live
//! database — only the pure helpers are covered here.

use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::McpResponseBuilder;
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::TextArtifact;

fn test_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("s"),
        TraceId::new("t"),
        ContextId::new("00000000-0000-4000-8000-000000000001"),
        AgentName::new("a"),
    )
}

#[test]
fn build_error_contains_message() {
    let result: rmcp::model::CallToolResult =
        McpResponseBuilder::<TextArtifact>::build_error("boom");
    let serialized = serde_json::to_string(&result).expect("serializable");
    assert!(serialized.contains("boom"));
}

#[test]
fn build_error_with_empty_message_still_returns_error() {
    let result: rmcp::model::CallToolResult = McpResponseBuilder::<TextArtifact>::build_error("");
    // Non-success result should be marked is_error=true.
    assert_eq!(result.is_error, Some(true));
}

#[test]
fn build_error_with_unicode_message() {
    let result: rmcp::model::CallToolResult =
        McpResponseBuilder::<TextArtifact>::build_error("失败 🚨");
    let serialized = serde_json::to_string(&result).expect("serializable");
    assert!(serialized.contains("失败"));
}

#[test]
fn builder_new_records_tool_name_in_debug() {
    let ctx = test_request_context();
    let exec_id = McpExecutionId::generate();
    let text = TextArtifact::new("payload", &ctx);
    let builder = McpResponseBuilder::new(text, "my-tool", &ctx, &exec_id);
    let debug = format!("{:?}", builder);
    assert!(debug.contains("my-tool"));
    assert!(debug.contains("McpResponseBuilder"));
}

#[test]
fn builder_new_accepts_string_tool_name() {
    let ctx = test_request_context();
    let exec_id = McpExecutionId::generate();
    let text = TextArtifact::new("payload", &ctx);
    let _ = McpResponseBuilder::new(text, String::from("dynamic-tool"), &ctx, &exec_id);
}

#[test]
fn build_error_long_message_preserved() {
    let long = "x".repeat(2000);
    let result: rmcp::model::CallToolResult =
        McpResponseBuilder::<TextArtifact>::build_error(long.clone());
    let serialized = serde_json::to_string(&result).expect("serializable");
    assert!(serialized.contains(&long));
}
