//! Tests for tool provider types

use systemprompt_provider_contracts::{
    ToolCallRequest, ToolCallResult, ToolContent, ToolContext, ToolDefinition, ToolProviderError,
};

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
    fn is_deserializable() {
        let json = r#"{"name":"tool","service_id":"svc","terminal_on_success":false}"#;
        let def: ToolDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "tool");
        assert_eq!(def.service_id, "svc");
    }

    #[test]
    fn is_clone() {
        let def = ToolDefinition::new("tool", "svc");
        let cloned = def.clone();
        assert_eq!(cloned.name, def.name);
    }

    #[test]
    fn is_eq() {
        let def1 = ToolDefinition::new("tool", "svc");
        let def2 = ToolDefinition::new("tool", "svc");
        assert_eq!(def1, def2);
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
    fn is_deserializable() {
        let req = test_request();
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ToolCallRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_call_id, req.tool_call_id);
    }

    #[test]
    fn is_clone() {
        let req = test_request();
        let cloned = req.clone();
        assert_eq!(cloned.name, req.name);
    }

    #[test]
    fn is_debug() {
        let req = test_request();
        let debug = format!("{:?}", req);
        assert!(debug.contains("ToolCallRequest"));
    }
}

mod tool_content_tests {
    use super::*;

    #[test]
    fn text_constructor() {
        let content = ToolContent::text("Hello");
        match content {
            ToolContent::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn text_variant() {
        let content = ToolContent::Text {
            text: "test".to_string(),
        };
        if let ToolContent::Text { text } = content {
            assert_eq!(text, "test");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn image_variant() {
        let content = ToolContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        if let ToolContent::Image { data, mime_type } = content {
            assert_eq!(data, "base64data");
            assert_eq!(mime_type, "image/png");
        } else {
            panic!("Expected Image variant");
        }
    }

    #[test]
    fn resource_variant() {
        let content = ToolContent::Resource {
            uri: "file://test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
        };
        if let ToolContent::Resource { uri, mime_type } = content {
            assert_eq!(uri, "file://test.txt");
            assert_eq!(mime_type, Some("text/plain".to_string()));
        } else {
            panic!("Expected Resource variant");
        }
    }

    #[test]
    fn resource_variant_without_mime() {
        let content = ToolContent::Resource {
            uri: "file://test.txt".to_string(),
            mime_type: None,
        };
        if let ToolContent::Resource { mime_type, .. } = content {
            assert!(mime_type.is_none());
        } else {
            panic!("Expected Resource variant");
        }
    }

    #[test]
    fn is_serializable() {
        let content = ToolContent::text("test");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("text"));
    }

    #[test]
    fn is_clone() {
        let content = ToolContent::text("test");
        let cloned = content.clone();
        if let ToolContent::Text { text } = cloned {
            assert_eq!(text, "test");
        }
    }

    #[test]
    fn is_debug() {
        let content = ToolContent::text("test");
        let debug = format!("{:?}", content);
        assert!(debug.contains("Text"));
    }
}

mod tool_call_result_tests {
    use super::*;

    #[test]
    fn success_creates_non_error() {
        let result = ToolCallResult::success("Done");
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn success_has_text_content() {
        let result = ToolCallResult::success("Done");
        assert_eq!(result.content.len(), 1);
        if let ToolContent::Text { text } = &result.content[0] {
            assert_eq!(text, "Done");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn success_has_no_structured_content() {
        let result = ToolCallResult::success("Done");
        assert!(result.structured_content.is_none());
    }

    #[test]
    fn success_has_no_meta() {
        let result = ToolCallResult::success("Done");
        assert!(result.meta.is_none());
    }

    #[test]
    fn error_creates_error_flag() {
        let result = ToolCallResult::error("Failed");
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn error_has_text_content() {
        let result = ToolCallResult::error("Failed");
        assert_eq!(result.content.len(), 1);
        if let ToolContent::Text { text } = &result.content[0] {
            assert_eq!(text, "Failed");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn with_structured_content() {
        let result =
            ToolCallResult::success("ok").with_structured_content(serde_json::json!({"key": 1}));
        assert!(result.structured_content.is_some());
        assert_eq!(result.structured_content.unwrap()["key"], 1);
    }

    #[test]
    fn is_serializable() {
        let result = ToolCallResult::success("test");
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("test"));
    }

    #[test]
    fn is_clone() {
        let result = ToolCallResult::success("test");
        let cloned = result.clone();
        assert_eq!(cloned.content.len(), result.content.len());
    }

    #[test]
    fn is_debug() {
        let result = ToolCallResult::success("test");
        let debug = format!("{:?}", result);
        assert!(debug.contains("ToolCallResult"));
    }
}

mod tool_context_tests {
    use super::*;

    #[test]
    fn new_sets_auth_token() {
        let ctx = ToolContext::new("token123");
        assert_eq!(ctx.auth_token, "token123");
    }

    #[test]
    fn new_defaults_session_id_to_none() {
        let ctx = ToolContext::new("token");
        assert!(ctx.session_id.is_none());
    }

    #[test]
    fn new_defaults_trace_id_to_none() {
        let ctx = ToolContext::new("token");
        assert!(ctx.trace_id.is_none());
    }

    #[test]
    fn new_defaults_ai_tool_call_id_to_none() {
        let ctx = ToolContext::new("token");
        assert!(ctx.ai_tool_call_id.is_none());
    }

    #[test]
    fn new_defaults_headers_to_empty() {
        let ctx = ToolContext::new("token");
        assert!(ctx.headers.is_empty());
    }

    #[test]
    fn with_session_id() {
        let ctx = ToolContext::new("token").with_session_id("sess-1");
        assert_eq!(ctx.session_id, Some("sess-1".to_string()));
    }

    #[test]
    fn with_trace_id() {
        let ctx = ToolContext::new("token").with_trace_id("trace-1");
        assert_eq!(ctx.trace_id, Some("trace-1".to_string()));
    }

    #[test]
    fn with_ai_tool_call_id() {
        let ctx = ToolContext::new("token").with_ai_tool_call_id("call-1");
        assert_eq!(ctx.ai_tool_call_id, Some("call-1".to_string()));
    }

    #[test]
    fn with_header() {
        let ctx = ToolContext::new("token").with_header("X-Custom", "value");
        assert_eq!(ctx.headers.get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn multiple_headers() {
        let ctx = ToolContext::new("token")
            .with_header("H1", "v1")
            .with_header("H2", "v2");
        assert_eq!(ctx.headers.len(), 2);
    }

    #[test]
    fn is_clone() {
        let ctx = ToolContext::new("token");
        let cloned = ctx.clone();
        assert_eq!(cloned.auth_token, ctx.auth_token);
    }

    #[test]
    fn is_debug() {
        let ctx = ToolContext::new("token");
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("ToolContext"));
    }
}

mod tool_provider_error_tests {
    use super::*;

    #[test]
    fn tool_not_found_contains_name() {
        let err = ToolProviderError::ToolNotFound("my_tool".to_string());
        assert!(err.to_string().contains("my_tool"));
    }

    #[test]
    fn service_not_found_contains_name() {
        let err = ToolProviderError::ServiceNotFound("svc-1".to_string());
        assert!(err.to_string().contains("svc-1"));
    }

    #[test]
    fn connection_failed_contains_service_and_message() {
        let err = ToolProviderError::ConnectionFailed {
            service: "svc".to_string(),
            message: "timeout".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("svc"));
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn execution_failed_contains_message() {
        let err = ToolProviderError::ExecutionFailed("crashed".to_string());
        assert!(err.to_string().contains("crashed"));
    }

    #[test]
    fn authorization_failed_contains_message() {
        let err = ToolProviderError::AuthorizationFailed("invalid token".to_string());
        assert!(err.to_string().contains("invalid token"));
    }

    #[test]
    fn configuration_error_contains_message() {
        let err = ToolProviderError::ConfigurationError("missing url".to_string());
        assert!(err.to_string().contains("missing url"));
    }

    #[test]
    fn internal_contains_message() {
        let err = ToolProviderError::Internal("unknown".to_string());
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ToolProviderError::ToolNotFound("t".to_string()));
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn is_debug() {
        let err = ToolProviderError::ToolNotFound("t".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ToolNotFound"));
    }
}
