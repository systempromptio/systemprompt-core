//! Classification and normalisation of tool results at the ingest narrow
//! waist: pure, no database.

use rmcp::model::{CallToolResult, ContentBlock, MetaObject, ResourceContents};
use serde_json::json;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::{IngestRequest, from_hook_failure, from_hook_response, from_wire_value};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{EXECUTION_META_KEY, ToolResultArtifact};
use systemprompt_models::mcp::ExecutionSource;

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s"),
        TraceId::new("t"),
        ContextId::generate(),
        AgentName::try_new("classify").expect("valid AgentName"),
    )
}

fn request(result: CallToolResult) -> IngestRequest {
    IngestRequest {
        result,
        tool_name: "Read".to_owned(),
        server_name: None,
        ai_tool_call_id: None,
        mcp_execution_id: None,
        ctx: ctx(),
        skill: None,
        source: ExecutionSource::HookClaudeCode,
        started_at: None,
        input: None,
    }
}

#[test]
fn hook_string_response_becomes_one_text_block() {
    let result = from_hook_response(&json!("file contents"));
    assert_eq!(result.content.len(), 1);
    assert!(result.structured_content.is_none());
    assert!(matches!(&result.content[0], ContentBlock::Text(t) if t.text == "file contents"));
}

#[test]
fn hook_mcp_shaped_response_is_parsed_as_the_wire_result() {
    let value = json!({
        "content": [{"type": "text", "text": "ok"}],
        "structuredContent": {"rows": 2},
        "isError": false,
        "_meta": {EXECUTION_META_KEY: {"mcp_execution_id": "exec-9", "artifact_id": "art-9"}}
    });
    let result = from_hook_response(&value);
    assert_eq!(result.structured_content, Some(json!({"rows": 2})));
    let meta = result.meta.expect("meta kept");
    assert_eq!(
        meta.0[EXECUTION_META_KEY]["mcp_execution_id"],
        json!("exec-9")
    );
}

#[test]
fn hook_object_response_without_mcp_shape_is_structured_output_only() {
    let result = from_hook_response(&json!({"file": {"path": "/x", "content": "y"}}));
    assert!(result.content.is_empty());
    assert_eq!(
        result.structured_content,
        Some(json!({"file": {"path": "/x", "content": "y"}}))
    );
    assert!(from_wire_value(&json!("text")).is_none());
}

#[test]
fn hook_failure_is_an_error_result() {
    let result = from_hook_failure("boom");
    assert_eq!(result.is_error, Some(true));
    assert!(matches!(&result.content[0], ContentBlock::Text(t) if t.text == "boom"));
}

#[test]
fn tool_result_artifact_reports_ui_resources() {
    let mut artifact = ToolResultArtifact::new("x");
    assert!(!artifact.has_ui_resource());
    artifact
        .blocks
        .push(systemprompt_models::artifacts::ToolResultBlock::Resource {
            uri: "ui://srv/artifact/1".to_owned(),
            mime_type: None,
            text: None,
            blob_byte_len: None,
            blob_sha256: None,
        });
    assert!(artifact.has_ui_resource());
}

#[test]
fn ingest_request_carries_a_ui_resource_result() {
    let mut result = CallToolResult::success(vec![ContentBlock::resource(
        ResourceContents::TextResourceContents {
            uri: "ui://srv/artifact/1".to_owned(),
            mime_type: Some("text/html".to_owned()),
            text: "<b>hi</b>".to_owned(),
            meta: None,
        },
    )]);
    result.meta = Some(MetaObject(
        json!({EXECUTION_META_KEY: {"artifact_id": "art-1"}})
            .as_object()
            .cloned()
            .unwrap(),
    ));
    let request = request(result);
    assert_eq!(request.source, ExecutionSource::HookClaudeCode);
    assert!(request.result.meta.is_some());
}

#[test]
fn execution_source_round_trips_and_maps_hosts() {
    for source in ExecutionSource::ALL {
        assert_eq!(ExecutionSource::parse(source.as_str()), Some(source));
    }
    assert_eq!(
        ExecutionSource::from_hook_host("opencode"),
        ExecutionSource::HookOpenCode
    );
    assert_eq!(
        ExecutionSource::from_hook_host("claude-code"),
        ExecutionSource::HookClaudeCode
    );
    assert!(ExecutionSource::Proxy.is_server_observed());
    assert!(!ExecutionSource::Gateway.is_server_observed());
}
