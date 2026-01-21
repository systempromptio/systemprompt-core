//! Tests for OpenAI response builder.

use std::time::Instant;
use systemprompt_ai::models::providers::openai::{
    OpenAiChoice, OpenAiPromptTokensDetails, OpenAiResponse, OpenAiResponseMessage, OpenAiUsage,
};
use systemprompt_ai::services::providers::openai::response_builder::build_response;
use uuid::Uuid;

fn create_test_response(
    content: Option<String>,
    finish_reason: Option<String>,
    usage: Option<OpenAiUsage>,
) -> OpenAiResponse {
    OpenAiResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1699000000,
        model: "gpt-4".to_string(),
        choices: vec![OpenAiChoice {
            index: 0,
            message: OpenAiResponseMessage {
                role: "assistant".to_string(),
                content,
                tool_calls: None,
            },
            finish_reason,
        }],
        usage,
    }
}

mod build_response_tests {
    use super::*;

    #[test]
    fn builds_response_with_content() {
        let request_id = Uuid::new_v4();
        let openai_response = create_test_response(
            Some("Hello, world!".to_string()),
            Some("stop".to_string()),
            Some(OpenAiUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                prompt_tokens_details: None,
            }),
        );

        let result = build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now());

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.content, "Hello, world!");
        assert_eq!(response.provider, "openai");
        assert_eq!(response.model, "gpt-4");
        assert_eq!(response.finish_reason, Some("stop".to_string()));
        assert_eq!(response.tokens_used, Some(15));
        assert_eq!(response.input_tokens, Some(10));
        assert_eq!(response.output_tokens, Some(5));
    }

    #[test]
    fn returns_error_for_empty_choices() {
        let request_id = Uuid::new_v4();
        let openai_response = OpenAiResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1699000000,
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: None,
        };

        let result = build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now());

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No response"));
    }

    #[test]
    fn handles_none_content() {
        let request_id = Uuid::new_v4();
        let openai_response = create_test_response(None, Some("stop".to_string()), None);

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert_eq!(result.content, "");
    }

    #[test]
    fn handles_empty_content() {
        let request_id = Uuid::new_v4();
        let openai_response =
            create_test_response(Some("".to_string()), Some("stop".to_string()), None);

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert_eq!(result.content, "");
    }

    #[test]
    fn handles_none_usage() {
        let request_id = Uuid::new_v4();
        let openai_response =
            create_test_response(Some("Hello".to_string()), Some("stop".to_string()), None);

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert_eq!(result.tokens_used, None);
        assert_eq!(result.input_tokens, None);
        assert_eq!(result.output_tokens, None);
    }

    #[test]
    fn detects_cache_hit() {
        let request_id = Uuid::new_v4();
        let openai_response = create_test_response(
            Some("Hello".to_string()),
            Some("stop".to_string()),
            Some(OpenAiUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                prompt_tokens_details: Some(OpenAiPromptTokensDetails {
                    cached_tokens: Some(80),
                }),
            }),
        );

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert!(result.cache_hit);
        assert_eq!(result.cache_read_tokens, Some(80));
    }

    #[test]
    fn no_cache_hit_when_zero_cached_tokens() {
        let request_id = Uuid::new_v4();
        let openai_response = create_test_response(
            Some("Hello".to_string()),
            Some("stop".to_string()),
            Some(OpenAiUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                prompt_tokens_details: Some(OpenAiPromptTokensDetails {
                    cached_tokens: Some(0),
                }),
            }),
        );

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert!(!result.cache_hit);
    }

    #[test]
    fn no_cache_hit_when_none_details() {
        let request_id = Uuid::new_v4();
        let openai_response = create_test_response(
            Some("Hello".to_string()),
            Some("stop".to_string()),
            Some(OpenAiUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                prompt_tokens_details: None,
            }),
        );

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert!(!result.cache_hit);
        assert_eq!(result.cache_read_tokens, None);
    }

    #[test]
    fn sets_is_streaming_to_false() {
        let request_id = Uuid::new_v4();
        let openai_response =
            create_test_response(Some("Hello".to_string()), Some("stop".to_string()), None);

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert!(!result.is_streaming);
    }

    #[test]
    fn calculates_latency() {
        let request_id = Uuid::new_v4();
        let openai_response =
            create_test_response(Some("Hello".to_string()), Some("stop".to_string()), None);

        let start = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result = build_response(request_id, &openai_response, "openai", "gpt-4", start).unwrap();

        assert!(result.latency_ms >= 10);
    }

    #[test]
    fn initializes_empty_tool_calls_and_results() {
        let request_id = Uuid::new_v4();
        let openai_response =
            create_test_response(Some("Hello".to_string()), Some("stop".to_string()), None);

        let result =
            build_response(request_id, &openai_response, "openai", "gpt-4", Instant::now()).unwrap();

        assert!(result.tool_calls.is_empty());
        assert!(result.tool_results.is_empty());
    }
}
