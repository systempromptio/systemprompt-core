use rmcp::model::{Annotated, RawContent, RawTextContent};
use serde_json::json;
use systemprompt_ai::models::tools::{CallToolResult, McpTool, ToolCall};
use systemprompt_ai::services::tools::{
    definition_to_mcp_tool, mcp_tool_to_definition, request_to_tool_call,
    rmcp_result_to_trait_result, trait_result_to_rmcp_result,
};
use systemprompt_identifiers::{AiToolCallId, McpServerId};
use systemprompt_models::ai::ToolModelConfig;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult as TraitToolCallResult, ToolContent, ToolDefinition,
};

mod request_to_tool_call_tests {
    use super::*;

    #[test]
    fn converts_basic_request() {
        let request = ToolCallRequest {
            tool_call_id: "call-abc".to_string(),
            name: "search".to_string(),
            arguments: json!({"query": "rust"}),
        };

        let tool_call = request_to_tool_call(&request);

        assert_eq!(tool_call.ai_tool_call_id.to_string(), "call-abc");
        assert_eq!(tool_call.name, "search");
        assert_eq!(tool_call.arguments["query"], "rust");
    }

    #[test]
    fn preserves_empty_arguments() {
        let request = ToolCallRequest {
            tool_call_id: "call-empty".to_string(),
            name: "no_args_tool".to_string(),
            arguments: json!({}),
        };

        let tool_call = request_to_tool_call(&request);

        assert!(tool_call.arguments.is_object());
        assert!(tool_call.arguments.as_object().unwrap().is_empty());
    }

    #[test]
    fn preserves_nested_arguments() {
        let request = ToolCallRequest {
            tool_call_id: "call-nested".to_string(),
            name: "complex".to_string(),
            arguments: json!({
                "level1": {
                    "level2": {
                        "value": 42
                    }
                },
                "array": [1, 2, 3]
            }),
        };

        let tool_call = request_to_tool_call(&request);

        assert_eq!(tool_call.arguments["level1"]["level2"]["value"], 42);
        assert_eq!(tool_call.arguments["array"][1], 2);
    }

    #[test]
    fn preserves_special_characters_in_name() {
        let request = ToolCallRequest {
            tool_call_id: "call-special".to_string(),
            name: "mcp-server:tool.action/v2".to_string(),
            arguments: json!({}),
        };

        let tool_call = request_to_tool_call(&request);

        assert_eq!(tool_call.name, "mcp-server:tool.action/v2");
    }

    #[test]
    fn roundtrip_with_tool_call_to_request() {
        use systemprompt_ai::services::tools::tool_call_to_request;

        let original = ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-rt"),
            name: "roundtrip_tool".to_string(),
            arguments: json!({"key": "value", "num": 99}),
        };

        let request = tool_call_to_request(&original);
        let converted = request_to_tool_call(&request);

        assert_eq!(original.name, converted.name);
        assert_eq!(original.arguments, converted.arguments);
    }
}

mod mcp_tool_with_model_config_tests {
    use super::*;

    #[test]
    fn converts_tool_with_model_config() {
        let mcp_tool = McpTool {
            name: "configurable_tool".to_string(),
            description: Some("A tool with config".to_string()),
            input_schema: None,
            output_schema: None,
            service_id: McpServerId::new("config-service"),
            terminal_on_success: false,
            model_config: Some(ToolModelConfig::new("anthropic", "claude-3")),
        };

        let definition = mcp_tool_to_definition(&mcp_tool);

        assert!(definition.model_config.is_some());
        let config_val = definition.model_config.unwrap();
        assert_eq!(config_val["provider"], "anthropic");
        assert_eq!(config_val["model"], "claude-3");
    }

    #[test]
    fn roundtrip_with_model_config() {
        let original = McpTool {
            name: "rt_config".to_string(),
            description: None,
            input_schema: None,
            output_schema: None,
            service_id: McpServerId::new("svc"),
            terminal_on_success: true,
            model_config: Some(
                ToolModelConfig::new("openai", "gpt-4").with_max_output_tokens(2048),
            ),
        };

        let definition = mcp_tool_to_definition(&original);
        let converted = definition_to_mcp_tool(&definition);

        assert_eq!(original.name, converted.name);
        assert_eq!(original.terminal_on_success, converted.terminal_on_success);
        let config = converted.model_config.unwrap();
        assert_eq!(config.provider, Some("openai".to_string()));
        assert_eq!(config.model, Some("gpt-4".to_string()));
        assert_eq!(config.max_output_tokens, Some(2048));
    }

    #[test]
    fn definition_with_invalid_model_config_returns_none() {
        let definition =
            ToolDefinition::new("test", "svc").with_model_config(json!("not_an_object"));

        let mcp_tool = definition_to_mcp_tool(&definition);

        assert!(mcp_tool.model_config.is_none());
    }
}

mod trait_result_roundtrip_tests {
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

    #[test]
    fn roundtrip_preserves_error_flag() {
        let original = CallToolResult::error(vec![create_text_content("failure")]);

        let trait_result = rmcp_result_to_trait_result(&original);
        let back = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(back.is_error, Some(true));
    }

    #[test]
    fn roundtrip_preserves_success_flag() {
        let original = CallToolResult::success(vec![create_text_content("ok")]);

        let trait_result = rmcp_result_to_trait_result(&original);
        let back = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(back.is_error, Some(false));
    }

    #[test]
    fn roundtrip_preserves_multiple_text_content() {
        let original = CallToolResult::success(vec![
            create_text_content("first"),
            create_text_content("second"),
            create_text_content("third"),
        ]);

        let trait_result = rmcp_result_to_trait_result(&original);
        let back = trait_result_to_rmcp_result(&trait_result);

        assert_eq!(back.content.len(), 3);
    }

    #[test]
    fn roundtrip_empty_content() {
        let original = CallToolResult::success(vec![]);

        let trait_result = rmcp_result_to_trait_result(&original);
        let back = trait_result_to_rmcp_result(&trait_result);

        assert!(back.content.is_empty());
        assert_eq!(back.is_error, Some(false));
    }

    #[test]
    fn trait_result_with_resource_converts_name_from_uri() {
        let trait_result = TraitToolCallResult {
            content: vec![ToolContent::Resource {
                uri: "https://example.com/data.json".to_string(),
                mime_type: Some("application/json".to_string()),
            }],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let rmcp = trait_result_to_rmcp_result(&trait_result);

        match &rmcp.content[0].raw {
            RawContent::ResourceLink(res) => {
                assert_eq!(res.uri, "https://example.com/data.json");
                assert_eq!(res.name, "https://example.com/data.json");
            },
            _ => panic!("Expected ResourceLink"),
        }
    }

    #[test]
    fn trait_result_with_none_mime_type_resource() {
        let trait_result = TraitToolCallResult {
            content: vec![ToolContent::Resource {
                uri: "file:///local.txt".to_string(),
                mime_type: None,
            }],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let rmcp = trait_result_to_rmcp_result(&trait_result);

        match &rmcp.content[0].raw {
            RawContent::ResourceLink(res) => {
                assert!(res.mime_type.is_none());
            },
            _ => panic!("Expected ResourceLink"),
        }
    }
}
