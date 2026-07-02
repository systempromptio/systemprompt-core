//! Unit tests for ArtifactBuilder.
//!
//! Target: crates/domain/agent/src/services/a2a_server/processing/artifact/mod.
//! rs

use rmcp::model::{CallToolResult, ContentBlock};
use systemprompt_agent::services::a2a_server::processing::ArtifactBuilder;
use systemprompt_identifiers::{AiToolCallId, ContextId, McpServerId, TaskId};
use systemprompt_models::{McpTool, ToolCall};

fn call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("c-{name}")),
        name: name.to_string(),
        arguments: serde_json::json!({}),
    }
}

fn result_no_struct() -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text("plain".to_string())])
}

fn result_null_struct() -> CallToolResult {
    let mut r = CallToolResult::success(vec![ContentBlock::text("text".to_string())]);
    r.structured_content = Some(serde_json::Value::Null);
    r
}

fn result_invalid_struct() -> CallToolResult {
    let mut r = CallToolResult::success(vec![ContentBlock::text("text".to_string())]);
    // Missing artifact_id / artifact / _metadata — parse_tool_response will fail.
    r.structured_content = Some(serde_json::json!({"unrelated": "field"}));
    r
}

fn empty_struct() -> CallToolResult {
    let mut r = CallToolResult::success(vec![ContentBlock::text("text".to_string())]);
    r.structured_content = Some(serde_json::json!({}));
    r
}

#[test]
fn build_artifacts_empty_lists_returns_empty() {
    let builder = ArtifactBuilder::new(
        vec![],
        vec![],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let artifacts = builder.build_artifacts().expect("ok");
    assert!(artifacts.is_empty());
}

#[test]
fn build_artifacts_skips_results_without_structured_content() {
    let builder = ArtifactBuilder::new(
        vec![call("a")],
        vec![result_no_struct()],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let artifacts = builder.build_artifacts().expect("ok");
    assert!(artifacts.is_empty());
}

#[test]
fn build_artifacts_skips_null_structured_content() {
    let builder = ArtifactBuilder::new(
        vec![call("a")],
        vec![result_null_struct()],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let artifacts = builder.build_artifacts().expect("ok");
    assert!(artifacts.is_empty());
}

#[test]
fn build_artifacts_fails_for_invalid_structured_content() {
    let builder = ArtifactBuilder::new(
        vec![call("a")],
        vec![result_invalid_struct()],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let err = builder
        .build_artifacts()
        .expect_err("transform should fail");
    assert!(err.to_string().contains("artifact transform failed"));
}

#[test]
fn build_artifacts_skips_when_tool_call_missing_for_index() {
    // result is at index 0 but tool_calls is empty -> get(0) returns None.
    let builder = ArtifactBuilder::new(
        vec![],
        vec![result_invalid_struct()],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let artifacts = builder.build_artifacts().expect("ok");
    assert!(artifacts.is_empty());
}

#[test]
fn build_artifacts_uses_output_schema_lookup_by_tool_name() {
    let mut r = result_invalid_struct();
    let schema = serde_json::json!({"type": "object"});
    let _ = &schema; // schema is captured by McpTool below.
    r.structured_content = Some(serde_json::json!({"bad": true}));

    let tool = McpTool::new("named", McpServerId::new("svc-1")).with_output_schema(schema);
    let builder = ArtifactBuilder::new(
        vec![ToolCall {
            ai_tool_call_id: AiToolCallId::new("c-named"),
            name: "named".to_string(),
            arguments: serde_json::json!({}),
        }],
        vec![r],
        vec![tool],
        ContextId::generate(),
        TaskId::generate(),
    );
    // Even with a schema, malformed structured_content yields error.
    let err = builder.build_artifacts().expect_err("invalid schema");
    assert!(err.to_string().contains("artifact transform failed"));
}

#[test]
fn build_artifacts_empty_structured_content_object_fails() {
    let builder = ArtifactBuilder::new(
        vec![call("a")],
        vec![empty_struct()],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let err = builder.build_artifacts().expect_err("empty object");
    assert!(err.to_string().contains("artifact transform failed"));
}

#[test]
fn build_artifacts_debug_impl_includes_struct_name() {
    let builder = ArtifactBuilder::new(
        vec![],
        vec![],
        vec![],
        ContextId::generate(),
        TaskId::generate(),
    );
    let s = format!("{:?}", builder);
    assert!(s.contains("ArtifactBuilder"));
}
