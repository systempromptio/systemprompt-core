// Full-path tests for McpToA2aTransformer::transform over the wire shape:
// typed structuredContent plus execution provenance under the
// io.systemprompt/execution _meta key.

use rmcp::model::{CallToolResult, MetaObject};
use serde_json::json;
use systemprompt_agent::models::a2a::Part;
use systemprompt_agent::services::mcp::artifact_transformer::{
    McpToA2aTransformer, TransformParams,
};
use systemprompt_models::artifacts::EXECUTION_META_KEY;

fn wire_result(artifact: serde_json::Value, exec_meta: serde_json::Value) -> CallToolResult {
    let mut result = CallToolResult::success(vec![]);
    result.structured_content = Some(artifact);
    let mut meta = serde_json::Map::new();
    meta.insert(EXECUTION_META_KEY.to_owned(), exec_meta);
    result.meta = Some(MetaObject(meta));
    result
}

fn valid_result() -> CallToolResult {
    wire_result(
        json!({"x-artifact-type": "text", "value": "hello"}),
        json!({
            "artifact_id": "art-xform-1",
            "mcp_execution_id": "exec-xform-1",
            "skill_id": "skill-7",
            "skill_name": "writer",
            "execution_id": "exec-ref-1"
        }),
    )
}

#[test]
fn transform_builds_artifact() {
    let result = valid_result();
    let artifact = McpToA2aTransformer::transform(&TransformParams {
        tool_name: "writer-tool",
        tool_result: &result,
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
    assert!(
        artifact
            .metadata
            .fingerprint
            .as_deref()
            .is_some_and(|fp| fp.starts_with("writer-tool-"))
    );
    assert_eq!(
        artifact.metadata.skill_id.as_ref().map(|s| s.as_str()),
        Some("skill-7")
    );
    assert_eq!(artifact.parts.len(), 1);
    assert!(matches!(artifact.parts[0], Part::Data(_)));
    assert_eq!(artifact.extensions.len(), 1);
    assert_eq!(
        artifact.extensions[0],
        json!(systemprompt_models::a2a::ARTIFACT_RENDERING_URI)
    );
}

#[test]
fn transform_missing_type_errors() {
    let result = wire_result(
        json!({"no": "type-hint"}),
        json!({"artifact_id": "art-2", "mcp_execution_id": "exec-2"}),
    );
    let outcome = McpToA2aTransformer::transform(&TransformParams {
        tool_name: "mystery",
        tool_result: &result,
        output_schema: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "t",
        tool_arguments: None,
    });
    assert!(outcome.is_err());
}

#[test]
fn transform_with_schema_type_hint() {
    let result = valid_result();
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
fn transform_without_execution_meta_errors() {
    let mut result = CallToolResult::success(vec![]);
    result.structured_content = Some(json!({"x-artifact-type": "text", "value": "v"}));
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
fn transform_falls_back_to_meta_execution_id() {
    let result = wire_result(
        json!({"x-artifact-type": "list", "items": []}),
        json!({
            "artifact_id": "art-3",
            "mcp_execution_id": "",
            "execution_id": "fallback-exec"
        }),
    );
    let artifact = McpToA2aTransformer::transform(&TransformParams {
        tool_name: "lister",
        tool_result: &result,
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
