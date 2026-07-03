//! Tests for MCP tool/result conversion into `systemprompt_traits` shapes.

use rmcp::model::{CallToolResult, ContentBlock};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::services::tool_provider::conversions::{to_tool_definition, to_tool_result};
use systemprompt_models::ai::tools::McpTool;
use systemprompt_traits::ToolContent;

#[test]
fn to_tool_definition_maps_all_fields() {
    let tool = McpTool {
        name: "create_task".to_owned(),
        description: Some("creates a task".to_owned()),
        input_schema: Some(serde_json::json!({"type": "object"})),
        output_schema: Some(serde_json::json!({"type": "string"})),
        service_id: McpServerId::new("tasks"),
        terminal_on_success: true,
        model_config: None,
    };

    let def = to_tool_definition(&tool);
    assert_eq!(def.name, "create_task");
    assert_eq!(def.description.as_deref(), Some("creates a task"));
    assert_eq!(def.service_id, "tasks");
    assert!(def.terminal_on_success);
    assert_eq!(
        def.input_schema,
        Some(serde_json::json!({"type": "object"}))
    );
    assert_eq!(
        def.output_schema,
        Some(serde_json::json!({"type": "string"}))
    );
    assert!(def.model_config.is_none());
}

#[test]
fn to_tool_result_maps_text_content() {
    let result = CallToolResult::success(vec![ContentBlock::text("hello")]);
    let converted = to_tool_result(&result);

    assert_eq!(converted.content.len(), 1);
    match &converted.content[0] {
        ToolContent::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected text content, got {other:?}"),
    }
    assert_eq!(converted.is_error, Some(false));
    assert!(converted.structured_content.is_none());
}

#[test]
fn to_tool_result_maps_image_and_error_flag() {
    let result = CallToolResult::error(vec![ContentBlock::image("aGVsbG8=", "image/png")]);
    let converted = to_tool_result(&result);

    assert_eq!(converted.content.len(), 1);
    match &converted.content[0] {
        ToolContent::Image { data, mime_type } => {
            assert_eq!(data, "aGVsbG8=");
            assert_eq!(mime_type, "image/png");
        },
        other => panic!("expected image content, got {other:?}"),
    }
    assert_eq!(converted.is_error, Some(true));
}
