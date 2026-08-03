//! Tests for SchemaTransformer.

use serde_json::json;
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::services::schema::{ProviderCapabilities, SchemaTransformer, TransformedTool};
use systemprompt_identifiers::McpServerId;

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
        let tool = create_test_tool("test", "Original description", json!({"type": "object"}));

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
        let err = result.unwrap_err();
        assert!(err.to_string().contains("missing"));
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
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
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
        result.unwrap_err();
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
        let tool = create_test_tool("original_name", "Description", discriminated_union_schema());

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
        let values: Vec<Option<&String>> = result
            .iter()
            .map(|t| t.discriminator_value.as_ref())
            .collect();

        assert!(
            values
                .iter()
                .any(|v| v.map(|s| s.as_str()) == Some("create"))
        );
        assert!(
            values
                .iter()
                .any(|v| v.map(|s| s.as_str()) == Some("delete"))
        );
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
            assert!(
                props.contains_key("common_field")
                    || props.contains_key("data")
                    || props.contains_key("id")
            );
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
            assert!(transformed.description.len() > "Base description".len());
        }
    }
}

mod function_name_tests {
    use super::*;

    #[test]
    fn preserves_tool_name() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::anthropic());
        let tool = create_test_tool("my_tool", "A test tool", json!({"type": "object"}));

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
}

mod variant_naming_and_required_merging {
    use super::*;

    fn union_with_base_required() -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string"},
                "common_field": {"type": "string"}
            },
            "required": ["action", "common_field"],
            "allOf": [
                {
                    "if": {"properties": {"action": {"const": "create"}}},
                    "then": {
                        "properties": {"data": {"type": "string"}},
                        "required": ["data", "common_field"]
                    }
                }
            ]
        })
    }

    #[test]
    fn the_discriminator_is_dropped_from_required_while_base_fields_survive() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool("act", "Acts", union_with_base_required());

        let result = transformer.transform(&tool).unwrap();
        let variant = result.first().expect("one variant");
        let required: Vec<&str> = variant.input_schema["required"]
            .as_array()
            .expect("required is an array")
            .iter()
            .map(|v| v.as_str().expect("required entries are strings"))
            .collect();

        assert!(
            !required.contains(&"action"),
            "the discriminator is pinned by the split, so it must not stay required: {required:?}"
        );
        assert!(
            required.contains(&"common_field"),
            "a base required field must survive the split: {required:?}"
        );
        assert!(
            required.contains(&"data"),
            "the variant's own required field must be merged in: {required:?}"
        );
        assert_eq!(
            required.iter().filter(|r| **r == "common_field").count(),
            1,
            "a field required by both base and variant must not be duplicated: {required:?}"
        );
    }

    fn union_with_discriminator(value: &str) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {"action": {"type": "string"}},
            "allOf": [
                {
                    "if": {"properties": {"action": {"const": value}}},
                    "then": {"properties": {"data": {"type": "string"}}}
                }
            ]
        })
    }

    #[test]
    fn characters_illegal_in_a_function_name_are_replaced_in_the_variant_name() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool("act", "Acts", union_with_discriminator("create/thing now"));

        let result = transformer.transform(&tool).unwrap();
        let name = &result.first().expect("one variant").name;

        assert!(
            !name.contains('/') && !name.contains(' '),
            "illegal characters must be replaced, got {name}"
        );
        assert_eq!(name, "act_create_thing_now");
        assert_eq!(
            result[0].discriminator_value.as_deref(),
            Some("create/thing now"),
            "the discriminator value itself is not sanitised — only the emitted name is"
        );
    }

    #[test]
    fn a_variant_name_is_truncated_to_the_provider_name_limit() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let long_value = "x".repeat(200);
        let tool = create_test_tool("act", "Acts", union_with_discriminator(&long_value));

        let result = transformer.transform(&tool).unwrap();
        let name = &result.first().expect("one variant").name;

        assert_eq!(
            name.len(),
            64,
            "the emitted name must be capped, got {name}"
        );
        assert!(name.starts_with("act_x"));
    }

    #[test]
    fn a_leading_digit_in_a_variant_name_is_prefixed_rather_than_dropped() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        // `sanitize_function_name` sees the whole `{tool}_{variant}` string, so
        // a tool whose own name starts with a digit drives the first-character
        // arms.
        let tool = create_test_tool("9act", "Acts", union_with_discriminator("create"));

        let result = transformer.transform(&tool).unwrap();
        let name = &result.first().expect("one variant").name;

        assert_eq!(name, "_9act_create");
    }

    #[test]
    fn a_leading_symbol_in_a_variant_name_becomes_an_underscore() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let tool = create_test_tool("+act", "Acts", union_with_discriminator("create"));

        let result = transformer.transform(&tool).unwrap();
        assert_eq!(result.first().expect("one variant").name, "_act_create");
    }

    #[test]
    fn a_split_candidate_with_an_empty_description_is_rejected() {
        let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
        let mut tool = create_test_tool("act", "Acts", union_with_discriminator("create"));
        tool.description = Some(String::new());

        let err = transformer
            .transform(&tool)
            .expect_err("a union tool with no description must not be split");
        assert!(err.to_string().contains("description"), "got {err}");
    }
}
