// Full-path tests for McpToA2aTransformer::transform / transform_from_json,
// which the helper-level transformer tests do not exercise.

use rmcp::model::CallToolResult;
use serde_json::json;
use systemprompt_agent::models::a2a::Part;
use systemprompt_agent::services::mcp::artifact_transformer::{
    McpToA2aTransformer, TransformFromJsonParams, TransformParams,
};

fn valid_tool_response() -> serde_json::Value {
    json!({
        "artifact_id": "art-xform-1",
        "mcp_execution_id": "exec-xform-1",
        "artifact": {"x-artifact-type": "text", "value": "hello"},
        "_metadata": {
            "skill_id": "skill-7",
            "skill_name": "writer",
            "execution_id": "exec-ref-1"
        }
    })
}

#[test]
fn transform_from_json_builds_artifact() {
    let body = valid_tool_response();
    let artifact = McpToA2aTransformer::transform_from_json(&TransformFromJsonParams {
        tool_name: "writer-tool",
        tool_result_json: &body,
        output_schema: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-xform",
        tool_arguments: Some(&json!({"q": 1})),
    })
    .expect("transform");

    assert_eq!(artifact.id.as_str(), "art-xform-1");
    assert_eq!(artifact.title.as_deref(), Some("writer-tool"));
    assert_eq!(artifact.metadata.artifact_type, "text");
    assert_eq!(
        artifact.metadata.mcp_execution_id.as_deref(),
        Some("exec-xform-1")
    );
    assert!(artifact.metadata.fingerprint.is_some());
    assert_eq!(
        artifact.metadata.skill_id.as_ref().map(|s| s.as_str()),
        Some("skill-7")
    );
    assert_eq!(artifact.parts.len(), 1);
    assert!(matches!(artifact.parts[0], Part::Data(_)));
    assert!(!artifact.extensions.is_empty());
}

#[test]
fn transform_from_json_missing_type_errors() {
    let body = json!({
        "artifact_id": "art-2",
        "mcp_execution_id": "exec-2",
        "artifact": {"no": "type-hint"},
        "_metadata": {}
    });
    let result = McpToA2aTransformer::transform_from_json(&TransformFromJsonParams {
        tool_name: "mystery",
        tool_result_json: &body,
        output_schema: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "t",
        tool_arguments: None,
    });
    assert!(result.is_err());
}

#[test]
fn transform_from_json_invalid_envelope_errors() {
    let body = json!({"not": "an envelope"});
    let result = McpToA2aTransformer::transform_from_json(&TransformFromJsonParams {
        tool_name: "tool",
        tool_result_json: &body,
        output_schema: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "t",
        tool_arguments: None,
    });
    assert!(result.is_err());
}

#[test]
fn transform_from_call_tool_result_with_structured_content() {
    let mut result = CallToolResult::success(vec![]);
    result.structured_content = Some(valid_tool_response());

    let artifact = McpToA2aTransformer::transform(&TransformParams {
        tool_name: "writer-tool",
        tool_result: &result,
        output_schema: Some(&json!({"x-artifact-type": "text"})),
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-xform",
        tool_arguments: None,
    })
    .expect("transform");
    assert_eq!(artifact.metadata.artifact_type, "text");
    assert_eq!(artifact.title.as_deref(), Some("writer-tool"));
}

#[test]
fn transform_without_structured_content_errors() {
    let result = CallToolResult::success(vec![]);
    let outcome = McpToA2aTransformer::transform(&TransformParams {
        tool_name: "tool",
        tool_result: &result,
        output_schema: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "t",
        tool_arguments: None,
    });
    assert!(outcome.is_err());
}

#[test]
fn transform_from_json_falls_back_to_metadata_execution_id() {
    // mcp_execution_id empty -> falls back to _metadata.execution_id.
    let body = json!({
        "artifact_id": "art-3",
        "mcp_execution_id": "",
        "artifact": {"x-artifact-type": "list", "items": []},
        "_metadata": {"execution_id": "fallback-exec"}
    });
    let artifact = McpToA2aTransformer::transform_from_json(&TransformFromJsonParams {
        tool_name: "lister",
        tool_result_json: &body,
        output_schema: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "t",
        tool_arguments: None,
    })
    .expect("transform");
    assert_eq!(
        artifact.metadata.mcp_execution_id.as_deref(),
        Some("fallback-exec")
    );
}
