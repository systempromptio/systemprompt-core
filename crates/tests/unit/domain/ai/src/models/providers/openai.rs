//! Tests for OpenAI model types.

use systemprompt_ai::models::providers::openai::{
    OpenAiChoice, OpenAiFunction, OpenAiFunctionCall, OpenAiJsonSchema, OpenAiMessage,
    OpenAiMessageContent, OpenAiModels, OpenAiRequest, OpenAiResponse, OpenAiResponseFormat,
    OpenAiResponseMessage, OpenAiTool, OpenAiToolCall, OpenAiUsage,
};

mod openai_models_tests {
    use super::*;

    #[test]
    fn default_models_have_correct_ids() {
        let models = OpenAiModels::default();

        assert_eq!(models.gpt4_turbo.id, "gpt-4-turbo");
        assert_eq!(models.gpt35_turbo.id, "gpt-3.5-turbo");
    }

    #[test]
    fn default_models_have_max_tokens() {
        let models = OpenAiModels::default();

        assert_eq!(models.gpt4_turbo.max_tokens, 128_000);
        assert_eq!(models.gpt35_turbo.max_tokens, 16385);
    }

    #[test]
    fn default_models_support_tools() {
        let models = OpenAiModels::default();

        assert!(models.gpt4_turbo.supports_tools);
        assert!(models.gpt35_turbo.supports_tools);
    }

    #[test]
    fn default_models_have_cost_per_1k_tokens() {
        let models = OpenAiModels::default();

        assert!(models.gpt4_turbo.cost_per_1k_tokens > 0.0);
        assert!(models.gpt35_turbo.cost_per_1k_tokens > 0.0);
        // GPT-4 should be more expensive
        assert!(models.gpt4_turbo.cost_per_1k_tokens > models.gpt35_turbo.cost_per_1k_tokens);
    }
}

mod openai_message_tests {
    use super::*;

    fn assert_text_content(content: &OpenAiMessageContent, expected: &str) {
        match content {
            OpenAiMessageContent::Text(text) => assert_eq!(text, expected),
            OpenAiMessageContent::Parts(_) => panic!("Expected Text, got Parts"),
        }
    }

    #[test]
    fn create_user_message() {
        let msg = OpenAiMessage {
            role: "user".to_string(),
            content: OpenAiMessageContent::Text("Hello!".to_string()),
        };

        assert_eq!(msg.role, "user");
        assert_text_content(&msg.content, "Hello!");
    }

    #[test]
    fn message_serialization() {
        let msg = OpenAiMessage {
            role: "assistant".to_string(),
            content: OpenAiMessageContent::Text("Hi there!".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assistant"));
        assert!(json.contains("Hi there!"));
    }

    #[test]
    fn message_deserialization() {
        let json = r#"{"role": "system", "content": "You are helpful."}"#;
        let msg: OpenAiMessage = serde_json::from_str(json).unwrap();

        assert_eq!(msg.role, "system");
        assert_text_content(&msg.content, "You are helpful.");
    }
}

mod openai_request_tests {
    use super::*;

    #[test]
    fn create_minimal_request() {
        let request = OpenAiRequest {
            model: "gpt-4".to_string(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: OpenAiMessageContent::Text("Hello".to_string()),
            }],
            temperature: None,
            top_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            max_tokens: None,
            tools: None,
            response_format: None,
            reasoning_effort: None,
        };

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
    }

    #[test]
    fn create_request_with_parameters() {
        let request = OpenAiRequest {
            model: "gpt-4-turbo".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            top_p: Some(0.9),
            presence_penalty: Some(0.1),
            frequency_penalty: Some(0.2),
            max_tokens: Some(4096),
            tools: None,
            response_format: None,
            reasoning_effort: None,
        };

        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.top_p, Some(0.9));
        assert_eq!(request.max_tokens, Some(4096));
    }

    #[test]
    fn request_with_tools() {
        let tool = OpenAiTool {
            r#type: "function".to_string(),
            function: OpenAiFunction {
                name: "get_weather".to_string(),
                description: Some("Get weather info".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                }),
            },
        };

        let request = OpenAiRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            top_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            max_tokens: None,
            tools: Some(vec![tool]),
            response_format: None,
            reasoning_effort: None,
        };

        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn request_serialization() {
        let request = OpenAiRequest {
            model: "gpt-4".to_string(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: OpenAiMessageContent::Text("Test".to_string()),
            }],
            temperature: Some(0.5),
            top_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            max_tokens: Some(1000),
            tools: None,
            response_format: None,
            reasoning_effort: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("0.5"));
        assert!(json.contains("1000"));
    }
}

mod openai_response_tests {
    use super::*;

    #[test]
    fn parse_simple_response() {
        let response = OpenAiResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1677652288,
            model: "gpt-4".to_string(),
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiResponseMessage {
                    role: "assistant".to_string(),
                    content: Some("Hello!".to_string()),
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(OpenAiUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                prompt_tokens_details: None,
            }),
        };

        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(
            response.choices[0].message.content,
            Some("Hello!".to_string())
        );
    }

    #[test]
    fn parse_response_with_tool_calls() {
        let response = OpenAiResponse {
            id: "chatcmpl-456".to_string(),
            object: "chat.completion".to_string(),
            created: 1677652300,
            model: "gpt-4".to_string(),
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![OpenAiToolCall {
                        id: "call_abc123".to_string(),
                        r#type: "function".to_string(),
                        function: OpenAiFunctionCall {
                            name: "get_weather".to_string(),
                            arguments: r#"{"location": "Paris"}"#.to_string(),
                        },
                    }]),
                },
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: None,
        };

        assert!(response.choices[0].message.content.is_none());
        assert!(response.choices[0].message.tool_calls.is_some());
        let tool_calls = response.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
    }

    #[test]
    fn response_serialization_roundtrip() {
        let response = OpenAiResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: Some(OpenAiUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                prompt_tokens_details: None,
            }),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: OpenAiResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.id, deserialized.id);
        assert_eq!(response.model, deserialized.model);
    }
}

mod openai_response_format_tests {
    use super::*;

    #[test]
    fn text_format_serialization() {
        let format = OpenAiResponseFormat::Text;
        let json = serde_json::to_string(&format).unwrap();
        assert!(json.contains("text"));
    }

    #[test]
    fn json_object_format_serialization() {
        let format = OpenAiResponseFormat::JsonObject;
        let json = serde_json::to_string(&format).unwrap();
        assert!(json.contains("json_object"));
    }

    #[test]
    fn json_schema_format_serialization() {
        let format = OpenAiResponseFormat::JsonSchema {
            json_schema: OpenAiJsonSchema {
                name: "person".to_string(),
                schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }),
                strict: Some(true),
            },
        };

        let json = serde_json::to_string(&format).unwrap();
        assert!(json.contains("json_schema"));
        assert!(json.contains("person"));
        assert!(json.contains("strict"));
    }

    #[test]
    fn json_schema_format_deserialization() {
        let json = r#"{
            "type": "json_schema",
            "json_schema": {
                "name": "test",
                "schema": {"type": "object"}
            }
        }"#;

        let format: OpenAiResponseFormat = serde_json::from_str(json).unwrap();
        match format {
            OpenAiResponseFormat::JsonSchema { json_schema } => {
                assert_eq!(json_schema.name, "test");
            }
            _ => panic!("Expected JsonSchema format"),
        }
    }
}

mod openai_usage_tests {
    use super::*;

    #[test]
    fn usage_is_copy() {
        let usage = OpenAiUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: None,
        };
        let copied = usage;
        assert_eq!(usage.total_tokens, copied.total_tokens);
    }

    #[test]
    fn usage_with_cached_tokens() {
        use systemprompt_ai::models::providers::openai::OpenAiPromptTokensDetails;

        let usage = OpenAiUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: Some(OpenAiPromptTokensDetails {
                cached_tokens: Some(80),
            }),
        };

        assert!(usage.prompt_tokens_details.is_some());
        assert_eq!(
            usage.prompt_tokens_details.unwrap().cached_tokens,
            Some(80)
        );
    }
}
