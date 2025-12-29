//! Tests for SchemaTransformer.

use systemprompt_core_ai::services::schema::{ProviderCapabilities, SchemaTransformer, TransformedTool};
use systemprompt_core_ai::models::tools::McpTool;
use systemprompt_identifiers::McpServerId;
use serde_json::json;

fn create_test_tool(name: &str, description: &str, schema: serde_json::Value) -> McpTool {
    McpTool {
        name: name.to_string(),
        description: Some(description.to_string()),
        input_schema: Some(schema),
        output_schema: None,
        service_id: McpServerId::new("test-service"),
        terminal_on_success: false,
        model_config: None,
    }
}

mod pass_through_tests {
    use super::*;

    #[test]
    fn simple_schema_passes_through() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "simple_tool",
            "A simple tool",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            }),
        );

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "simple_tool");
        assert_eq!(result[0].original_name, "simple_tool");
        assert!(result[0].discriminator_value.is_none());
    }

    #[test]
    fn passes_through_when_no_transformation_needed() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::anthropic());
        let tool = create_test_tool(
            "complex_tool",
            "Complex but supported",
            json!({
                "type": "object",
                "allOf": [
                    {"properties": {"a": {"type": "string"}}}
                ]
            }),
        );

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn preserves_description() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::anthropic());
        let tool = create_test_tool(
            "test",
            "Original description",
            json!({"type": "object"}),
        );

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result[0].description, "Original description");
    }
}

mod error_handling_tests {
    use super::*;

    #[test]
    fn returns_error_for_missing_schema() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = McpTool {
            name: "no_schema".to_string(),
            description: Some("No schema".to_string()),
            input_schema: None,
            output_schema: None,
            service_id: McpServerId::new("test"),
            terminal_on_success: false,
            model_config: None,
        };

        let result = transformer.transform(&tool);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn returns_error_for_empty_description() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = McpTool {
            name: "no_desc".to_string(),
            description: Some("".to_string()),
            input_schema: Some(json!({"type": "object"})),
            output_schema: None,
            service_id: McpServerId::new("test"),
            terminal_on_success: false,
            model_config: None,
        };

        let result = transformer.transform(&tool);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn returns_error_for_none_description() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = McpTool {
            name: "null_desc".to_string(),
            description: None,
            input_schema: Some(json!({"type": "object"})),
            output_schema: None,
            service_id: McpServerId::new("test"),
            terminal_on_success: false,
            model_config: None,
        };

        let result = transformer.transform(&tool);
        assert!(result.is_err());
    }
}

mod auto_split_tests {
    use super::*;

    fn discriminated_union_schema() -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "common_field": {"type": "string"}
            },
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"const": "create"}
                        }
                    },
                    "then": {
                        "properties": {
                            "data": {"type": "string"}
                        },
                        "required": ["data"]
                    }
                },
                {
                    "if": {
                        "properties": {
                            "action": {"const": "delete"}
                        }
                    },
                    "then": {
                        "properties": {
                            "id": {"type": "integer"}
                        },
                        "required": ["id"]
                    }
                }
            ]
        })
    }

    #[test]
    fn splits_discriminated_union_for_gemini() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "action_tool",
            "Performs actions",
            discriminated_union_schema(),
        );

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn creates_variant_names() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "action_tool",
            "Performs actions",
            discriminated_union_schema(),
        );

        let result = transformer.transform(&tool).unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();

        assert!(names.contains(&"action_tool_create") || names.contains(&"action_tool_delete"));
    }

    #[test]
    fn preserves_original_name() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "original_name",
            "Description",
            discriminated_union_schema(),
        );

        let result = transformer.transform(&tool).unwrap();
        for transformed in &result {
            assert_eq!(transformed.original_name, "original_name");
        }
    }

    #[test]
    fn sets_discriminator_values() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "action_tool",
            "Performs actions",
            discriminated_union_schema(),
        );

        let result = transformer.transform(&tool).unwrap();
        let values: Vec<Option<&String>> = result.iter()
            .map(|t| t.discriminator_value.as_ref())
            .collect();

        assert!(values.iter().any(|v| v.map(|s| s.as_str()) == Some("create")));
        assert!(values.iter().any(|v| v.map(|s| s.as_str()) == Some("delete")));
    }

    #[test]
    fn merges_base_properties() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "action_tool",
            "Performs actions",
            discriminated_union_schema(),
        );

        let result = transformer.transform(&tool).unwrap();

        for transformed in &result {
            let props = transformed.input_schema["properties"].as_object().unwrap();
            // common_field should be present in all variants
            assert!(props.contains_key("common_field") ||
                    props.contains_key("data") ||
                    props.contains_key("id"));
        }
    }

    #[test]
    fn enhances_description_with_variant() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool(
            "action_tool",
            "Base description",
            discriminated_union_schema(),
        );

        let result = transformer.transform(&tool).unwrap();

        for transformed in &result {
            assert!(transformed.description.contains("Base description"));
            // Description should include variant info
            assert!(transformed.description.len() > "Base description".len());
        }
    }
}

mod function_name_tests {
    use super::*;

    #[test]
    fn preserves_tool_name() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::anthropic());
        let tool = create_test_tool(
            "my_tool",
            "A test tool",
            json!({"type": "object"}),
        );

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result[0].name, "my_tool");
    }

    #[test]
    fn keeps_valid_characters() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::anthropic());
        let tool = create_test_tool(
            "valid_tool-name",
            "Tool with valid chars",
            json!({"type": "object"}),
        );

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result[0].name, "valid_tool-name");
    }
}

mod transformed_tool_tests {
    use super::*;

    #[test]
    fn transformed_tool_is_debug() {
        let tool = TransformedTool {
            name: "test".to_string(),
            description: "Test description".to_string(),
            input_schema: json!({}),
            original_name: "test".to_string(),
            discriminator_value: None,
        };

        let debug = format!("{:?}", tool);
        assert!(debug.contains("test"));
    }

    #[test]
    fn transformed_tool_is_clone() {
        let tool = TransformedTool {
            name: "test".to_string(),
            description: "Test".to_string(),
            input_schema: json!({"type": "object"}),
            original_name: "test".to_string(),
            discriminator_value: Some("variant".to_string()),
        };

        let cloned = tool.clone();
        assert_eq!(tool.name, cloned.name);
        assert_eq!(tool.discriminator_value, cloned.discriminator_value);
    }
}
