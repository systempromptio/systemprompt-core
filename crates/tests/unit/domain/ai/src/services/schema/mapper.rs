//! Tests for ToolNameMapper.

use systemprompt_ai::services::schema::{ToolNameMapper, TransformedTool};
use serde_json::json;

fn create_transformed_tool(
    name: &str,
    original_name: &str,
    discriminator_value: Option<&str>,
) -> TransformedTool {
    TransformedTool {
        name: name.to_string(),
        description: "Test tool".to_string(),
        input_schema: json!({"type": "object"}),
        original_name: original_name.to_string(),
        discriminator_value: discriminator_value.map(String::from),
    }
}

mod new_tests {
    use super::*;

    #[test]
    fn creates_empty_mapper() {
        let mapper = ToolNameMapper::new();
        assert!(!mapper.is_variant("any_name"));
    }

    #[test]
    fn default_creates_same_as_new() {
        let mapper1 = ToolNameMapper::new();
        let mapper2 = ToolNameMapper::default();

        assert!(!mapper1.is_variant("test"));
        assert!(!mapper2.is_variant("test"));
    }
}

mod register_transformation_tests {
    use super::*;

    #[test]
    fn registers_simple_transformation() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("action_tool_create", "action_tool", Some("create"));

        mapper.register_transformation(&tool, Some("action".to_string()));

        assert!(mapper.is_variant("action_tool_create"));
    }

    #[test]
    fn registers_multiple_variants() {
        let mut mapper = ToolNameMapper::new();
        let tool1 = create_transformed_tool("action_tool_create", "action_tool", Some("create"));
        let tool2 = create_transformed_tool("action_tool_delete", "action_tool", Some("delete"));

        mapper.register_transformation(&tool1, Some("action".to_string()));
        mapper.register_transformation(&tool2, Some("action".to_string()));

        assert!(mapper.is_variant("action_tool_create"));
        assert!(mapper.is_variant("action_tool_delete"));
    }

    #[test]
    fn uses_default_discriminator_field() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("tool_variant", "tool", Some("variant"));

        // Pass None to use default "action" field
        mapper.register_transformation(&tool, None);

        // Should still be registered
        assert!(mapper.is_variant("tool_variant"));
    }

    #[test]
    fn registers_tool_without_discriminator() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("simple_tool", "simple_tool", None);

        mapper.register_transformation(&tool, None);

        assert!(mapper.is_variant("simple_tool"));
    }
}

mod resolve_tool_call_tests {
    use super::*;

    #[test]
    fn resolves_variant_to_original_name() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("action_tool_create", "action_tool", Some("create"));

        mapper.register_transformation(&tool, Some("action".to_string()));

        let params = json!({"data": "test"});
        let (resolved_name, _) = mapper.resolve_tool_call("action_tool_create", params);

        assert_eq!(resolved_name, "action_tool");
    }

    #[test]
    fn adds_discriminator_value_to_params() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("action_tool_create", "action_tool", Some("create"));

        mapper.register_transformation(&tool, Some("action".to_string()));

        let params = json!({"data": "test"});
        let (_, resolved_params) = mapper.resolve_tool_call("action_tool_create", params);

        assert_eq!(resolved_params["action"], "create");
        assert_eq!(resolved_params["data"], "test");
    }

    #[test]
    fn preserves_existing_params() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("tool_variant", "tool", Some("variant"));

        mapper.register_transformation(&tool, Some("type".to_string()));

        let params = json!({
            "name": "test",
            "count": 42,
            "nested": {"key": "value"}
        });
        let (_, resolved_params) = mapper.resolve_tool_call("tool_variant", params);

        assert_eq!(resolved_params["name"], "test");
        assert_eq!(resolved_params["count"], 42);
        assert_eq!(resolved_params["nested"]["key"], "value");
        assert_eq!(resolved_params["type"], "variant");
    }

    #[test]
    fn returns_original_for_unknown_variant() {
        let mapper = ToolNameMapper::new();
        let params = json!({"data": "test"});

        let (resolved_name, resolved_params) = mapper.resolve_tool_call("unknown_tool", params.clone());

        assert_eq!(resolved_name, "unknown_tool");
        assert_eq!(resolved_params, params);
    }

    #[test]
    fn handles_tool_without_discriminator_value() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("passthrough_tool", "passthrough_tool", None);

        mapper.register_transformation(&tool, None);

        let params = json!({"data": "test"});
        let (resolved_name, resolved_params) = mapper.resolve_tool_call("passthrough_tool", params);

        assert_eq!(resolved_name, "passthrough_tool");
        // No discriminator value should be added
        assert!(resolved_params.get("action").is_none());
    }

    #[test]
    fn handles_non_object_params() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("tool_v1", "tool", Some("v1"));

        mapper.register_transformation(&tool, Some("version".to_string()));

        // Pass a non-object value
        let params = json!("string value");
        let (resolved_name, resolved_params) = mapper.resolve_tool_call("tool_v1", params);

        assert_eq!(resolved_name, "tool");
        // Can't add discriminator to non-object, but should still work
        assert_eq!(resolved_params, "string value");
    }
}

mod get_variants_tests {
    use super::*;

    #[test]
    fn returns_all_variants_for_original() {
        let mut mapper = ToolNameMapper::new();
        let tool1 = create_transformed_tool("action_tool_create", "action_tool", Some("create"));
        let tool2 = create_transformed_tool("action_tool_delete", "action_tool", Some("delete"));
        let tool3 = create_transformed_tool("action_tool_update", "action_tool", Some("update"));

        mapper.register_transformation(&tool1, Some("action".to_string()));
        mapper.register_transformation(&tool2, Some("action".to_string()));
        mapper.register_transformation(&tool3, Some("action".to_string()));

        let variants = mapper.get_variants("action_tool").unwrap();

        assert_eq!(variants.len(), 3);
        assert!(variants.contains(&"action_tool_create".to_string()));
        assert!(variants.contains(&"action_tool_delete".to_string()));
        assert!(variants.contains(&"action_tool_update".to_string()));
    }

    #[test]
    fn returns_none_for_unknown_original() {
        let mapper = ToolNameMapper::new();
        assert!(mapper.get_variants("unknown").is_none());
    }

    #[test]
    fn returns_single_variant() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("only_variant", "original", Some("only"));

        mapper.register_transformation(&tool, None);

        let variants = mapper.get_variants("original").unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0], "only_variant");
    }
}

mod is_variant_tests {
    use super::*;

    #[test]
    fn returns_true_for_registered_variant() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("variant_name", "original", Some("v1"));

        mapper.register_transformation(&tool, None);

        assert!(mapper.is_variant("variant_name"));
    }

    #[test]
    fn returns_false_for_unregistered_name() {
        let mapper = ToolNameMapper::new();
        assert!(!mapper.is_variant("unknown"));
    }

    #[test]
    fn returns_false_for_original_name() {
        let mut mapper = ToolNameMapper::new();
        let tool = create_transformed_tool("variant_name", "original_name", Some("v1"));

        mapper.register_transformation(&tool, None);

        // Original name is not in forward map
        assert!(!mapper.is_variant("original_name"));
    }
}

mod multiple_tools_tests {
    use super::*;

    #[test]
    fn handles_multiple_different_tools() {
        let mut mapper = ToolNameMapper::new();

        let tool1 = create_transformed_tool("search_web", "search", Some("web"));
        let tool2 = create_transformed_tool("search_local", "search", Some("local"));
        let tool3 = create_transformed_tool("action_run", "action", Some("run"));
        let tool4 = create_transformed_tool("action_stop", "action", Some("stop"));

        mapper.register_transformation(&tool1, Some("type".to_string()));
        mapper.register_transformation(&tool2, Some("type".to_string()));
        mapper.register_transformation(&tool3, Some("command".to_string()));
        mapper.register_transformation(&tool4, Some("command".to_string()));

        // Check variants for search
        let search_variants = mapper.get_variants("search").unwrap();
        assert_eq!(search_variants.len(), 2);

        // Check variants for action
        let action_variants = mapper.get_variants("action").unwrap();
        assert_eq!(action_variants.len(), 2);

        // Resolve search_web
        let (name, params) = mapper.resolve_tool_call("search_web", json!({"query": "test"}));
        assert_eq!(name, "search");
        assert_eq!(params["type"], "web");

        // Resolve action_stop
        let (name, params) = mapper.resolve_tool_call("action_stop", json!({}));
        assert_eq!(name, "action");
        assert_eq!(params["command"], "stop");
    }
}
