//! Tests for shared response builder.

use std::time::Instant;
use systemprompt_ai::services::providers::shared::{
    build_response, BuildResponseParams, TokenUsage,
};
use uuid::Uuid;

mod token_usage_tests {
    use super::*;

    #[test]
    fn default_has_none_values() {
        let usage = TokenUsage::default();
        assert!(usage.tokens_used.is_none());
        assert!(usage.input_tokens.is_none());
        assert!(usage.output_tokens.is_none());
        assert!(!usage.cache_hit);
        assert!(usage.cache_read_tokens.is_none());
        assert!(usage.cache_creation_tokens.is_none());
    }

    #[test]
    fn can_set_all_fields() {
        let usage = TokenUsage {
            tokens_used: Some(1500),
            input_tokens: Some(1000),
            output_tokens: Some(500),
            cache_hit: true,
            cache_read_tokens: Some(200),
            cache_creation_tokens: Some(50),
        };

        assert_eq!(usage.tokens_used, Some(1500));
        assert_eq!(usage.input_tokens, Some(1000));
        assert_eq!(usage.output_tokens, Some(500));
        assert!(usage.cache_hit);
        assert_eq!(usage.cache_read_tokens, Some(200));
        assert_eq!(usage.cache_creation_tokens, Some(50));
    }

    #[test]
    fn is_copy() {
        let usage = TokenUsage {
            tokens_used: Some(100),
            ..TokenUsage::default()
        };
        let copied = usage;
        assert_eq!(usage.tokens_used, copied.tokens_used);
    }

    #[test]
    fn is_clone() {
        let usage = TokenUsage {
            tokens_used: Some(100),
            ..TokenUsage::default()
        };
        let cloned = usage.clone();
        assert_eq!(usage.tokens_used, cloned.tokens_used);
    }

    #[test]
    fn is_debug() {
        let usage = TokenUsage::default();
        let debug_str = format!("{:?}", usage);
        assert!(debug_str.contains("TokenUsage"));
    }
}

mod build_response_params_tests {
    use super::*;

    #[test]
    fn can_create_params() {
        let request_id = Uuid::new_v4();
        let start = Instant::now();

        let params = BuildResponseParams {
            request_id,
            content: "Hello".to_string(),
            provider: "test",
            model: "test-model",
            finish_reason: Some("stop".to_string()),
            usage: TokenUsage::default(),
            start,
        };

        assert_eq!(params.request_id, request_id);
        assert_eq!(params.content, "Hello");
        assert_eq!(params.provider, "test");
        assert_eq!(params.model, "test-model");
    }

    #[test]
    fn is_debug() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "provider",
            model: "model",
            finish_reason: None,
            usage: TokenUsage::default(),
            start: Instant::now(),
        };

        let debug_str = format!("{:?}", params);
        assert!(debug_str.contains("BuildResponseParams"));
    }
}

mod build_response_tests {
    use super::*;

    #[test]
    fn builds_response_with_all_fields() {
        let request_id = Uuid::new_v4();
        let start = Instant::now();

        let params = BuildResponseParams {
            request_id,
            content: "Hello, world!".to_string(),
            provider: "anthropic",
            model: "claude-3",
            finish_reason: Some("end_turn".to_string()),
            usage: TokenUsage {
                tokens_used: Some(1500),
                input_tokens: Some(1000),
                output_tokens: Some(500),
                cache_hit: true,
                cache_read_tokens: Some(100),
                cache_creation_tokens: Some(50),
            },
            start,
        };

        let response = build_response(params);

        assert_eq!(response.request_id, request_id);
        assert_eq!(response.content, "Hello, world!");
        assert_eq!(response.provider, "anthropic");
        assert_eq!(response.model, "claude-3");
        assert_eq!(response.finish_reason, Some("end_turn".to_string()));
        assert_eq!(response.tokens_used, Some(1500));
        assert_eq!(response.input_tokens, Some(1000));
        assert_eq!(response.output_tokens, Some(500));
        assert!(response.cache_hit);
        assert_eq!(response.cache_read_tokens, Some(100));
        assert_eq!(response.cache_creation_tokens, Some(50));
    }

    #[test]
    fn sets_is_streaming_to_false() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "test",
            model: "test",
            finish_reason: None,
            usage: TokenUsage::default(),
            start: Instant::now(),
        };

        let response = build_response(params);

        assert!(!response.is_streaming);
    }

    #[test]
    fn initializes_empty_tool_calls() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "test",
            model: "test",
            finish_reason: None,
            usage: TokenUsage::default(),
            start: Instant::now(),
        };

        let response = build_response(params);

        assert!(response.tool_calls.is_empty());
        assert!(response.tool_results.is_empty());
    }

    #[test]
    fn calculates_latency_from_start() {
        let start = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(5));

        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "test",
            model: "test",
            finish_reason: None,
            usage: TokenUsage::default(),
            start,
        };

        let response = build_response(params);

        assert!(response.latency_ms >= 5);
    }

    #[test]
    fn handles_none_finish_reason() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "test",
            model: "test",
            finish_reason: None,
            usage: TokenUsage::default(),
            start: Instant::now(),
        };

        let response = build_response(params);

        assert!(response.finish_reason.is_none());
    }

    #[test]
    fn handles_empty_content() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "".to_string(),
            provider: "test",
            model: "test",
            finish_reason: None,
            usage: TokenUsage::default(),
            start: Instant::now(),
        };

        let response = build_response(params);

        assert_eq!(response.content, "");
    }

    #[test]
    fn converts_provider_and_model_to_owned_strings() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "borrowed_provider",
            model: "borrowed_model",
            finish_reason: None,
            usage: TokenUsage::default(),
            start: Instant::now(),
        };

        let response = build_response(params);

        assert_eq!(response.provider, "borrowed_provider".to_string());
        assert_eq!(response.model, "borrowed_model".to_string());
    }

    #[test]
    fn handles_no_cache_hit() {
        let params = BuildResponseParams {
            request_id: Uuid::new_v4(),
            content: "Test".to_string(),
            provider: "test",
            model: "test",
            finish_reason: None,
            usage: TokenUsage {
                cache_hit: false,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                ..TokenUsage::default()
            },
            start: Instant::now(),
        };

        let response = build_response(params);

        assert!(!response.cache_hit);
        assert!(response.cache_read_tokens.is_none());
        assert!(response.cache_creation_tokens.is_none());
    }
}
