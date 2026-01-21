//! Tests for tool adapter functions.

use rmcp::model::{Annotated, RawContent, RawImageContent, RawResource, RawTextContent};
use serde_json::json;
use systemprompt_ai::models::tools::{CallToolResult, McpTool, ToolCall};
use systemprompt_ai::services::tools::{
    definition_to_mcp_tool, mcp_tool_to_definition, rmcp_result_to_trait_result,
    tool_call_to_request, trait_result_to_rmcp_result,
};
use systemprompt_identifiers::{AiToolCallId, McpServerId};
use systemprompt_traits::{ToolCallResult as TraitToolCallResult, ToolContent, ToolDefinition};

mod mcp_tool_to_definition_tests {
    use super::*;

    fn create_test_mcp_tool() -> McpTool {
        McpTool {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })),
            output_schema: Some(json!({"type": "string"})),
            service_id: McpServerId::new("test-service"),
            terminal_on_success: true,
            model_config: None,
        }
    }

    #[test]
    fn converts_basic_fields() {
        let mcp_tool = create_test_mcp_tool();
        let definition = mcp_tool_to_definition(&mcp_tool);

        assert_eq!(definition.name, "test_tool");
        assert_eq!(definition.description, Some("A test tool".to_string()));
        assert_eq!(definition.service_id, "test-service");
        assert!(definition.terminal_on_success);
    }

    #[test]
    fn converts_input_schema() {
        let mcp_tool = create_test_mcp_tool();
        let definition = mcp_tool_to_definition(&mcp_tool);

        assert!(definition.input_schema.is_some());
        let schema = definition.input_schema.unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
    }

    #[test]
    fn converts_output_schema() {
        let mcp_tool = create_test_mcp_tool();
        let definition = mcp_tool_to_definition(&mcp_tool);

        assert!(definition.output_schema.is_some());
        assert_eq!(definition.output_schema.unwrap()["type"], "string");
    }

    #[test]
    fn handles_none_schemas() {
        let mcp_tool = McpTool {
            name: "minimal".to_string(),
            description: None,
            input_schema: None,
            output_schema: None,
            service_id: McpServerId::new("service"),
            terminal_on_success: false,
            model_config: None,
        };

        let definition = mcp_tool_to_definition(&mcp_tool);

        assert!(definition.description.is_none());
        assert!(definition.input_schema.is_none());
        assert!(definition.output_schema.is_none());
    }
}

mod definition_to_mcp_tool_tests {
    use super::*;

    fn create_test_definition() -> ToolDefinition {
        ToolDefinition {
            name: "definition_tool".to_string(),
            description: Some("A defined tool".to_string()),
            input_schema: Some(json!({"type": "object"})),
            output_schema: None,
            service_id: "def-service".to_string(),
            terminal_on_success: false,
            model_config: None,
        }
    }

    #[test]
    fn converts_basic_fields() {
        let definition = create_test_definition();
        let mcp_tool = definition_to_mcp_tool(&definition);

        assert_eq!(mcp_tool.name, "definition_tool");
        assert_eq!(mcp_tool.description, Some("A defined tool".to_string()));
        assert_eq!(mcp_tool.service_id.to_string(), "def-service");
        assert!(!mcp_tool.terminal_on_success);
    }

    #[test]
    fn converts_schemas() {
        let definition = create_test_definition();
        let mcp_tool = definition_to_mcp_tool(&definition);

        assert!(mcp_tool.input_schema.is_some());
        assert!(mcp_tool.output_schema.is_none());
    }

    #[test]
    fn roundtrip_conversion() {
        let original = McpTool {
            name: "roundtrip".to_string(),
            description: Some("Test roundtrip".to_string()),
            input_schema: Some(json!({"type": "object", "properties": {}})),
            output_schema: Some(json!({"type": "array"})),
            service_id: McpServerId::new("roundtrip-service"),
            terminal_on_success: true,
            model_config: None,
        };

        let definition = mcp_tool_to_definition(&original);
        let converted = definition_to_mcp_tool(&definition);

        assert_eq!(original.name, converted.name);
        assert_eq!(original.description, converted.description);
        assert_eq!(original.terminal_on_success, converted.terminal_on_success);
    }
}

mod tool_call_to_request_tests {
    use super::*;

    #[test]
    fn converts_tool_call() {
        let call = ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-123"),
            name: "search".to_string(),
            arguments: json!({"query": "test"}),
        };

        let request = tool_call_to_request(&call);

        assert_eq!(request.tool_call_id, "call-123");
        assert_eq!(request.name, "search");
        assert_eq!(request.arguments["query"], "test");
    }

    #[test]
    fn preserves_complex_arguments() {
        let call = ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-456"),
            name: "complex_tool".to_string(),
            arguments: json!({
                "nested": {
                    "array": [1, 2, 3],
                    "bool": true
                },
                "simple": "value"
            }),
        };

        let request = tool_call_to_request(&call);

        assert_eq!(request.arguments["nested"]["array"][0], 1);
        assert_eq!(request.arguments["nested"]["bool"], true);
        assert_eq!(request.arguments["simple"], "value");
    }

    #[test]
    fn handles_empty_arguments() {
        let call = ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-empty"),
            name: "no_args".to_string(),
            arguments: json!({}),
        };

        let request = tool_call_to_request(&call);

        assert!(request.arguments.is_object());
        assert!(request.arguments.as_object().unwrap().is_empty());
    }
}

mod rmcp_result_to_trait_result_tests {
    use super::*;

    fn create_text_content(text: &str) -> Annotated<RawContent> {
        Annotated {
            raw: RawContent::Text(RawTextContent {
                text: text.to_string(),
                meta: None,
            }),
            annotations: None,
        }
    }

    fn create_image_content() -> Annotated<RawContent> {
        Annotated {
            raw: RawContent::Image(RawImageContent {
                data: "base64data".to_string(),
                mime_type: "image/png".to_string(),
                meta: None,
            }),
            annotations: None,
        }
    }

    fn create_resource_content() -> Annotated<RawContent> {
        Annotated {
            raw: RawContent::ResourceLink(RawResource {
                uri: "file:///test.txt".to_string(),
                name: "test.txt".to_string(),
                title: None,
                description: None,
                mime_type: Some("text/plain".to_string()),
                size: None,
                icons: None,
                meta: None,
            }),
            annotations: None,
        }
    }

    #[test]
    fn converts_text_content() {
        let result = CallToolResult {
            content: vec![create_text_content("Hello")],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&result);

        assert_eq!(trait_result.content.len(), 1);
        match &trait_result.content[0] {
            ToolContent::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn converts_image_content() {
        let result = CallToolResult {
            content: vec![create_image_content()],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&result);

        assert_eq!(trait_result.content.len(), 1);
        match &trait_result.content[0] {
            ToolContent::Image { data, mime_type } => {
                assert_eq!(data, "base64data");
                assert_eq!(mime_type, "image/png");
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn converts_resource_content() {
        let result = CallToolResult {
            content: vec![create_resource_content()],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&result);

        assert_eq!(trait_result.content.len(), 1);
        match &trait_result.content[0] {
            ToolContent::Resource { uri, mime_type } => {
                assert_eq!(uri, "file:///test.txt");
                assert_eq!(mime_type, &Some("text/plain".to_string()));
            }
            _ => panic!("Expected Resource content"),
        }
    }

    #[test]
    fn preserves_structured_content() {
        let result = CallToolResult {
            content: vec![],
            structured_content: Some(json!({"key": "value"})),
            is_error: Some(false),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&result);

        assert_eq!(trait_result.structured_content, Some(json!({"key": "value"})));
    }

    #[test]
    fn preserves_is_error() {
        let result = CallToolResult {
            content: vec![create_text_content("Error occurred")],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&result);

        assert_eq!(trait_result.is_error, Some(true));
    }

    #[test]
    fn handles_multiple_content_types() {
        let result = CallToolResult {
            content: vec![create_text_content("Hello"), create_image_content()],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&result);

        assert_eq!(trait_result.content.len(), 2);
    }
}

mod trait_result_to_rmcp_result_tests {
    use super::*;

    #[test]
    fn converts_text_content() {
        let trait_result = TraitToolCallResult {
            content: vec![ToolContent::Text {
                text: "Hello".to_string(),
            }],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let result = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(result.content.len(), 1);
        match &result.content[0].raw {
            RawContent::Text(text) => assert_eq!(text.text, "Hello"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn converts_image_content() {
        let trait_result = TraitToolCallResult {
            content: vec![ToolContent::Image {
                data: "base64data".to_string(),
                mime_type: "image/jpeg".to_string(),
            }],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let result = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(result.content.len(), 1);
        match &result.content[0].raw {
            RawContent::Image(img) => {
                assert_eq!(img.data, "base64data");
                assert_eq!(img.mime_type, "image/jpeg");
            }
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn converts_resource_content() {
        let trait_result = TraitToolCallResult {
            content: vec![ToolContent::Resource {
                uri: "file:///resource.txt".to_string(),
                mime_type: Some("text/plain".to_string()),
            }],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let result = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(result.content.len(), 1);
        match &result.content[0].raw {
            RawContent::ResourceLink(res) => {
                assert_eq!(res.uri, "file:///resource.txt");
                assert_eq!(res.mime_type, Some("text/plain".to_string()));
            }
            _ => panic!("Expected ResourceLink content"),
        }
    }

    #[test]
    fn preserves_structured_content() {
        let trait_result = TraitToolCallResult {
            content: vec![],
            structured_content: Some(json!({"data": [1, 2, 3]})),
            is_error: Some(false),
            meta: None,
        };

        let result = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(result.structured_content, Some(json!({"data": [1, 2, 3]})));
    }

    #[test]
    fn preserves_is_error() {
        let trait_result = TraitToolCallResult {
            content: vec![],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        };

        let result = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn roundtrip_preserves_content() {
        let original = CallToolResult {
            content: vec![
                Annotated {
                    raw: RawContent::Text(RawTextContent {
                        text: "roundtrip".to_string(),
                        meta: None,
                    }),
                    annotations: None,
                },
            ],
            structured_content: Some(json!({"test": true})),
            is_error: Some(false),
            meta: None,
        };

        let trait_result = rmcp_result_to_trait_result(&original);
        let back = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(back.structured_content, original.structured_content);
        assert_eq!(back.is_error, original.is_error);
    }
}

