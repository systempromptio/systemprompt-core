//! Tests for ToolDefinition and ToolCallRequest.

use systemprompt_provider_contracts::{ToolCallRequest, ToolDefinition};

mod tool_definition_tests {
    use super::*;

    #[test]
    fn new_sets_name_and_service_id() {
        let def = ToolDefinition::new("my_tool", "service-1");
        assert_eq!(def.name, "my_tool");
        assert_eq!(def.service_id, "service-1");
    }

    #[test]
    fn new_defaults_description_to_none() {
        let def = ToolDefinition::new("tool", "svc");
        assert!(def.description.is_none());
    }

    #[test]
    fn new_defaults_input_schema_to_none() {
        let def = ToolDefinition::new("tool", "svc");
        assert!(def.input_schema.is_none());
    }

    #[test]
    fn new_defaults_output_schema_to_none() {
        let def = ToolDefinition::new("tool", "svc");
        assert!(def.output_schema.is_none());
    }

    #[test]
    fn new_defaults_terminal_on_success_to_false() {
        let def = ToolDefinition::new("tool", "svc");
        assert!(!def.terminal_on_success);
    }

    #[test]
    fn new_defaults_model_config_to_none() {
        let def = ToolDefinition::new("tool", "svc");
        assert!(def.model_config.is_none());
    }

    #[test]
    fn with_description() {
        let def = ToolDefinition::new("tool", "svc").with_description("A test tool");
        assert_eq!(def.description, Some("A test tool".to_string()));
    }

    #[test]
    fn with_input_schema() {
        let schema = serde_json::json!({"type": "object"});
        let def = ToolDefinition::new("tool", "svc").with_input_schema(schema.clone());
        assert_eq!(def.input_schema, Some(schema));
    }

    #[test]
    fn with_output_schema() {
        let schema = serde_json::json!({"type": "string"});
        let def = ToolDefinition::new("tool", "svc").with_output_schema(schema.clone());
        assert_eq!(def.output_schema, Some(schema));
    }

    #[test]
    fn with_terminal_on_success_true() {
        let def = ToolDefinition::new("tool", "svc").with_terminal_on_success(true);
        assert!(def.terminal_on_success);
    }

    #[test]
    fn with_terminal_on_success_false() {
        let def = ToolDefinition::new("tool", "svc").with_terminal_on_success(false);
        assert!(!def.terminal_on_success);
    }

    #[test]
    fn with_model_config() {
        let config = serde_json::json!({"temperature": 0.7});
        let def = ToolDefinition::new("tool", "svc").with_model_config(config.clone());
        assert_eq!(def.model_config, Some(config));
    }

    #[test]
    fn is_serializable() {
        let def = ToolDefinition::new("test", "svc");
        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("svc"));
    }

    #[test]
    fn is_debug() {
        let def = ToolDefinition::new("tool", "svc");
        let debug = format!("{:?}", def);
        assert!(debug.contains("ToolDefinition"));
    }
}

mod tool_call_request_tests {
    use super::*;

    fn test_request() -> ToolCallRequest {
        ToolCallRequest {
            tool_call_id: "call-1".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
        }
    }

    #[test]
    fn has_tool_call_id() {
        let req = test_request();
        assert_eq!(req.tool_call_id, "call-1");
    }

    #[test]
    fn has_name() {
        let req = test_request();
        assert_eq!(req.name, "search");
    }

    #[test]
    fn has_arguments() {
        let req = test_request();
        assert_eq!(req.arguments["query"], "test");
    }

    #[test]
    fn is_serializable() {
        let req = test_request();
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("call-1"));
        assert!(json.contains("search"));
    }

    #[test]
    fn is_debug() {
        let req = test_request();
        let debug = format!("{:?}", req);
        assert!(debug.contains("ToolCallRequest"));
    }
}
