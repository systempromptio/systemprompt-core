//! Unit tests for `McpResponseBuilder` static helpers and constructor.
//! Full `build()` exercises the artifact repository, which requires a live
//! database — only the pure helpers are covered here.

use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::{ClientProfile, McpResponseBuilder, ToolIdentity};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::TextArtifact;

fn test_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("s"),
        TraceId::new("t"),
        ContextId::new_unchecked("00000000-0000-4000-8000-000000000001"),
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
    let text = TextArtifact::new("payload");
    let builder = McpResponseBuilder::new(
        text,
        ToolIdentity::new("my-server", "my-tool"),
        &ctx,
        &exec_id,
        &ClientProfile::unknown(),
    );
    let debug = format!("{:?}", builder);
    assert!(debug.contains("my-tool"));
    assert!(debug.contains("McpResponseBuilder"));
}

#[test]
fn builder_new_accepts_string_identity() {
    let ctx = test_request_context();
    let exec_id = McpExecutionId::generate();
    let text = TextArtifact::new("payload");
    let _ = McpResponseBuilder::new(
        text,
        ToolIdentity::new(String::from("dynamic-server"), String::from("dynamic-tool")),
        &ctx,
        &exec_id,
        &ClientProfile::unknown(),
    );
}

#[test]
fn build_error_long_message_preserved() {
    let long = "x".repeat(2000);
    let result: rmcp::model::CallToolResult =
        McpResponseBuilder::<TextArtifact>::build_error(long.clone());
    let serialized = serde_json::to_string(&result).expect("serializable");
    assert!(serialized.contains(&long));
}

// Pins the `tools/call` result envelope. rmcp 3.0 added the SEP-2663
// `resultType` discriminator and its constructors populate it, so every result
// we emit carries it — a visible wire change for strict downstream consumers.
#[test]
fn tool_result_envelope_carries_result_type_discriminator() {
    let result: rmcp::model::CallToolResult =
        McpResponseBuilder::<TextArtifact>::build_error("boom");
    let json = serde_json::to_value(&result).expect("serializable");

    assert_eq!(json["resultType"], "complete");
    assert_eq!(json["isError"], true);

    let round_tripped: rmcp::model::CallToolResult =
        serde_json::from_value(json).expect("round-trips");
    assert_eq!(round_tripped.result_type, result.result_type);
}

// A pre-3.0 result has no `resultType`; it must still deserialize — the field
// stays `None` — so persisted artifacts and older peers keep working.
#[test]
fn tool_result_without_result_type_still_deserializes() {
    let legacy = serde_json::json!({
        "content": [{ "type": "text", "text": "hello" }],
        "isError": false,
    });

    let result: rmcp::model::CallToolResult =
        serde_json::from_value(legacy).expect("legacy envelope deserializes");

    assert_eq!(result.result_type, None);
    assert_eq!(result.is_error, Some(false));
}
