//! Tests for Anthropic model types.

use systemprompt_core_ai::models::providers::anthropic::{
    AnthropicContent, AnthropicContentBlock, AnthropicMessage, AnthropicModels, AnthropicRequest,
    AnthropicResponse, AnthropicTool, AnthropicToolChoice, AnthropicUsage,
};

mod anthropic_models_tests {
    use super::*;

    #[test]
    fn default_models_have_correct_ids() {
        let models = AnthropicModels::default();

        assert!(models.opus.id.contains("opus"));
        assert!(models.sonnet.id.contains("sonnet"));
        assert!(models.haiku.id.contains("haiku"));
    }

    #[test]
    fn default_models_have_max_tokens() {
        let models = AnthropicModels::default();

        assert_eq!(models.opus.max_tokens, 200_000);
        assert_eq!(models.sonnet.max_tokens, 200_000);
        assert_eq!(models.haiku.max_tokens, 200_000);
    }

    #[test]
    fn default_models_support_tools() {
        let models = AnthropicModels::default();

        assert!(models.opus.supports_tools);
        assert!(models.sonnet.supports_tools);
        assert!(models.haiku.supports_tools);
    }

    #[test]
    fn models_have_decreasing_cost() {
        let models = AnthropicModels::default();

        // Opus is most expensive, Haiku is cheapest
        assert!(models.opus.cost_per_1k_tokens > models.sonnet.cost_per_1k_tokens);
        assert!(models.sonnet.cost_per_1k_tokens > models.haiku.cost_per_1k_tokens);
    }
}

mod anthropic_message_tests {
    use super::*;

    #[test]
    fn create_message_with_text_content() {
        let msg = AnthropicMessage {
            role: "user".to_string(),
            content: AnthropicContent::Text("Hello!".to_string()),
        };

        assert_eq!(msg.role, "user");
        match msg.content {
            AnthropicContent::Text(text) => assert_eq!(text, "Hello!"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn create_message_with_blocks() {
        let blocks = vec![AnthropicContentBlock::Text {
            text: "First block".to_string(),
        }];

        let msg = AnthropicMessage {
            role: "assistant".to_string(),
            content: AnthropicContent::Blocks(blocks),
        };

        match msg.content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn message_serialization_with_text() {
        let msg = AnthropicMessage {
            role: "user".to_string(),
            content: AnthropicContent::Text("Test message".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Test message"));
    }

    #[test]
    fn message_deserialization_with_text() {
        let json = r#"{"role": "assistant", "content": "Hello there!"}"#;
        let msg: AnthropicMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.role, "assistant");
        match msg.content {
            AnthropicContent::Text(text) => assert_eq!(text, "Hello there!"),
            _ => panic!("Expected Text content"),
        }
    }
}

mod anthropic_content_block_tests {
    use super::*;

    #[test]
    fn text_block() {
        let block = AnthropicContentBlock::Text {
            text: "Hello world".to_string(),
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("text"));
        assert!(json.contains("Hello world"));
    }

    #[test]
    fn tool_use_block() {
        let block = AnthropicContentBlock::ToolUse {
            id: "tool_123".to_string(),
            name: "calculator".to_string(),
            input: serde_json::json!({"x": 5, "y": 3}),
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("tool_use"));
        assert!(json.contains("tool_123"));
        assert!(json.contains("calculator"));
    }

    #[test]
    fn tool_result_block() {
        let block = AnthropicContentBlock::ToolResult {
            tool_use_id: "tool_123".to_string(),
            content: "Result: 8".to_string(),
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("tool_result"));
        assert!(json.contains("tool_123"));
        assert!(json.contains("Result: 8"));
    }

    #[test]
    fn deserialize_text_block() {
        let json = r#"{"type": "text", "text": "Hello"}"#;
        let block: AnthropicContentBlock = serde_json::from_str(json).unwrap();

        match block {
            AnthropicContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn deserialize_tool_use_block() {
        let json = r#"{"type": "tool_use", "id": "abc", "name": "test", "input": {}}"#;
        let block: AnthropicContentBlock = serde_json::from_str(json).unwrap();

        match block {
            AnthropicContentBlock::ToolUse { id, name, .. } => {
                assert_eq!(id, "abc");
                assert_eq!(name, "test");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }
}

mod anthropic_request_tests {
    use super::*;

    #[test]
    fn create_minimal_request() {
        let request = AnthropicRequest {
            model: "claude-3-opus".to_string(),
            messages: vec![],
            max_tokens: 1024,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            system: None,
            tools: None,
            tool_choice: None,
            stream: None,
            thinking: None,
        };

        assert_eq!(request.model, "claude-3-opus");
        assert_eq!(request.max_tokens, 1024);
    }

    #[test]
    fn request_with_system_prompt() {
        let request = AnthropicRequest {
            model: "claude-3-sonnet".to_string(),
            messages: vec![],
            max_tokens: 2048,
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
            stop_sequences: None,
            system: Some("You are a helpful assistant.".to_string()),
            tools: None,
            tool_choice: None,
            stream: None,
            thinking: None,
        };

        assert_eq!(
            request.system,
            Some("You are a helpful assistant.".to_string())
        );
    }

    #[test]
    fn request_with_tools() {
        let tool = AnthropicTool {
            name: "search".to_string(),
            description: Some("Search the web".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
        };

        let request = AnthropicRequest {
            model: "claude-3-opus".to_string(),
            messages: vec![],
            max_tokens: 1024,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            system: None,
            tools: Some(vec![tool]),
            tool_choice: Some(AnthropicToolChoice::Auto),
            stream: None,
            thinking: None,
        };

        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn request_serialization() {
        let request = AnthropicRequest {
            model: "claude-3-haiku".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::Text("Hello".to_string()),
            }],
            max_tokens: 512,
            temperature: Some(0.5),
            top_p: None,
            top_k: Some(40),
            stop_sequences: Some(vec!["END".to_string()]),
            system: None,
            tools: None,
            tool_choice: None,
            stream: None,
            thinking: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("claude-3-haiku"));
        assert!(json.contains("512"));
        assert!(json.contains("0.5"));
        assert!(json.contains("40"));
        assert!(json.contains("END"));
    }
}

mod anthropic_tool_choice_tests {
    use super::*;

    #[test]
    fn auto_serialization() {
        let choice = AnthropicToolChoice::Auto;
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("auto"));
    }

    #[test]
    fn any_serialization() {
        let choice = AnthropicToolChoice::Any;
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("any"));
    }

    #[test]
    fn tool_serialization() {
        let choice = AnthropicToolChoice::Tool {
            name: "specific_tool".to_string(),
        };
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("tool"));
        assert!(json.contains("specific_tool"));
    }
}

mod anthropic_response_tests {
    use super::*;

    #[test]
    fn parse_simple_response() {
        let response = AnthropicResponse {
            id: "msg_123".to_string(),
            r#type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![AnthropicContentBlock::Text {
                text: "Hello!".to_string(),
            }],
            model: "claude-3-opus".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input: 10,
                output: 5,
                cache_creation: None,
                cache_read: None,
            },
        };

        assert_eq!(response.id, "msg_123");
        assert_eq!(response.content.len(), 1);
        assert_eq!(response.usage.input, 10);
        assert_eq!(response.usage.output, 5);
    }

    #[test]
    fn parse_response_with_tool_use() {
        let response = AnthropicResponse {
            id: "msg_456".to_string(),
            r#type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![AnthropicContentBlock::ToolUse {
                id: "tool_abc".to_string(),
                name: "calculator".to_string(),
                input: serde_json::json!({"operation": "add"}),
            }],
            model: "claude-3-sonnet".to_string(),
            stop_reason: Some("tool_use".to_string()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input: 20,
                output: 10,
                cache_creation: None,
                cache_read: None,
            },
        };

        assert_eq!(response.stop_reason, Some("tool_use".to_string()));
        match &response.content[0] {
            AnthropicContentBlock::ToolUse { name, .. } => {
                assert_eq!(name, "calculator");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn response_serialization_roundtrip() {
        let response = AnthropicResponse {
            id: "test".to_string(),
            r#type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![],
            model: "claude-3".to_string(),
            stop_reason: None,
            stop_sequence: None,
            usage: AnthropicUsage {
                input: 100,
                output: 50,
                cache_creation: Some(10),
                cache_read: Some(80),
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: AnthropicResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.id, deserialized.id);
        assert_eq!(response.usage.cache_creation, deserialized.usage.cache_creation);
    }
}

mod anthropic_usage_tests {
    use super::*;

    #[test]
    fn usage_is_copy() {
        let usage = AnthropicUsage {
            input: 100,
            output: 50,
            cache_creation: None,
            cache_read: None,
        };
        let copied = usage;
        assert_eq!(usage.input, copied.input);
    }

    #[test]
    fn usage_with_cache_info() {
        let usage = AnthropicUsage {
            input: 100,
            output: 50,
            cache_creation: Some(20),
            cache_read: Some(80),
        };

        assert_eq!(usage.cache_creation, Some(20));
        assert_eq!(usage.cache_read, Some(80));
    }

    #[test]
    fn usage_serialization() {
        let usage = AnthropicUsage {
            input: 100,
            output: 50,
            cache_creation: Some(10),
            cache_read: None,
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("input_tokens"));
        assert!(json.contains("output_tokens"));
        assert!(json.contains("cache_creation_input_tokens"));
    }
}
