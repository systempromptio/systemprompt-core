//! Tests for OpenAI converter functions.

use serde_json::json;
use systemprompt_core_ai::services::providers::openai::converters::{
    convert_response_format, convert_tools,
};
use systemprompt_core_ai::models::tools::McpTool;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::ai::ResponseFormat;

fn create_mcp_tool(
    name: &str,
    description: Option<&str>,
    input_schema: Option<serde_json::Value>,
) -> McpTool {
    McpTool {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        input_schema,
        output_schema: None,
        service_id: McpServerId::new("test-service"),
        terminal_on_success: false,
        model_config: None,
    }
}

mod convert_tools_tests {
    use super::*;

    #[test]
    fn converts_tool_with_all_fields() {
        let tool = create_mcp_tool(
            "test_tool",
            Some("Test description"),
            Some(json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })),
        );

        let result = convert_tools(vec![tool]).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].r#type, "function");
        assert_eq!(result[0].function.name, "test_tool");
        assert_eq!(
            result[0].function.description,
            Some("Test description".to_string())
        );
        assert_eq!(
            result[0].function.parameters,
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })
        );
    }

    #[test]
    fn converts_multiple_tools() {
        let tools = vec![
            create_mcp_tool("tool1", Some("First tool"), Some(json!({"type": "object"}))),
            create_mcp_tool("tool2", Some("Second tool"), Some(json!({"type": "object"}))),
        ];

        let result = convert_tools(tools).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].function.name, "tool1");
        assert_eq!(result[1].function.name, "tool2");
    }

    #[test]
    fn returns_error_for_missing_input_schema() {
        let tool = create_mcp_tool("no_schema_tool", Some("Missing schema"), None);

        let result = convert_tools(vec![tool]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("no_schema_tool"));
        assert!(err.to_string().contains("input_schema"));
    }

    #[test]
    fn handles_tool_without_description() {
        let tool = create_mcp_tool("no_desc_tool", None, Some(json!({"type": "object"})));

        let result = convert_tools(vec![tool]).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].function.description, None);
    }

    #[test]
    fn returns_empty_vec_for_empty_input() {
        let result = convert_tools(vec![]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn preserves_complex_schema() {
        let complex_schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "minLength": 1},
                "count": {"type": "integer", "minimum": 0},
                "tags": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            },
            "required": ["name"]
        });

        let tool = create_mcp_tool(
            "complex_tool",
            Some("Complex schema tool"),
            Some(complex_schema.clone()),
        );

        let result = convert_tools(vec![tool]).unwrap();

        assert_eq!(result[0].function.parameters, complex_schema);
    }
}

mod convert_response_format_tests {
    use super::*;

    #[test]
    fn text_format_returns_none() {
        let format = ResponseFormat::Text;
        let result = convert_response_format(&format).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn json_object_format_returns_json_object() {
        let format = ResponseFormat::JsonObject;
        let result = convert_response_format(&format).unwrap();

        assert!(result.is_some());
        // Check it serializes correctly
        let serialized = serde_json::to_value(&result.unwrap()).unwrap();
        assert_eq!(serialized["type"], "json_object");
    }

    #[test]
    fn json_schema_format_returns_json_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "response": {"type": "string"}
            }
        });

        let format = ResponseFormat::JsonSchema {
            schema: schema.clone(),
            name: Some("test_schema".to_string()),
            strict: Some(true),
        };

        let result = convert_response_format(&format).unwrap();

        assert!(result.is_some());
        let serialized = serde_json::to_value(&result.unwrap()).unwrap();
        assert_eq!(serialized["type"], "json_schema");
        assert_eq!(serialized["json_schema"]["name"], "test_schema");
        assert_eq!(serialized["json_schema"]["strict"], true);
    }

    #[test]
    fn json_schema_requires_name() {
        let format = ResponseFormat::JsonSchema {
            schema: json!({"type": "object"}),
            name: None,
            strict: Some(true),
        };

        let result = convert_response_format(&format);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn json_schema_with_strict_false() {
        let format = ResponseFormat::JsonSchema {
            schema: json!({"type": "object"}),
            name: Some("test".to_string()),
            strict: Some(false),
        };

        let result = convert_response_format(&format).unwrap();

        let serialized = serde_json::to_value(&result.unwrap()).unwrap();
        assert_eq!(serialized["json_schema"]["strict"], false);
    }

    #[test]
    fn json_schema_with_none_strict() {
        let format = ResponseFormat::JsonSchema {
            schema: json!({"type": "object"}),
            name: Some("test".to_string()),
            strict: None,
        };

        let result = convert_response_format(&format).unwrap();

        let serialized = serde_json::to_value(&result.unwrap()).unwrap();
        // None should be serialized as null or omitted
        assert!(
            serialized["json_schema"]["strict"].is_null()
                || !serialized["json_schema"]
                    .as_object()
                    .unwrap()
                    .contains_key("strict")
        );
    }
}
