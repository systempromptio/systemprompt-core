//! Tests for ChatRole, ChatMessage, SamplingParameters, and TokenUsage.

use systemprompt_provider_contracts::{
    ChatMessage, ChatRole, SamplingParameters, TokenUsage,
};

mod chat_role_tests {
    use super::*;

    #[test]
    fn system_serializes_lowercase() {
        let json = serde_json::to_string(&ChatRole::System).unwrap();
        assert_eq!(json, "\"system\"");
    }

    #[test]
    fn user_serializes_lowercase() {
        let json = serde_json::to_string(&ChatRole::User).unwrap();
        assert_eq!(json, "\"user\"");
    }

    #[test]
    fn assistant_serializes_lowercase() {
        let json = serde_json::to_string(&ChatRole::Assistant).unwrap();
        assert_eq!(json, "\"assistant\"");
    }

    #[test]
    fn tool_serializes_lowercase() {
        let json = serde_json::to_string(&ChatRole::Tool).unwrap();
        assert_eq!(json, "\"tool\"");
    }

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", ChatRole::Assistant);
        assert!(debug.contains("Assistant"));
    }
}

mod chat_message_tests {
    use super::*;

    #[test]
    fn user_constructor() {
        let msg = ChatMessage::user("Hello");
        assert_eq!(msg.role, ChatRole::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn assistant_constructor() {
        let msg = ChatMessage::assistant("Hi there");
        assert_eq!(msg.role, ChatRole::Assistant);
        assert_eq!(msg.content, "Hi there");
    }

    #[test]
    fn system_constructor() {
        let msg = ChatMessage::system("You are helpful");
        assert_eq!(msg.role, ChatRole::System);
        assert_eq!(msg.content, "You are helpful");
    }

    #[test]
    fn is_serializable() {
        let msg = ChatMessage::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("test"));
    }

    #[test]
    fn is_debug() {
        let msg = ChatMessage::user("test");
        let debug = format!("{:?}", msg);
        assert!(debug.contains("ChatMessage"));
    }
}

mod sampling_parameters_tests {
    use super::*;

    #[test]
    fn new_has_no_temperature() {
        let params = SamplingParameters::new();
        assert!(params.temperature.is_none());
    }

    #[test]
    fn new_has_no_top_p() {
        let params = SamplingParameters::new();
        assert!(params.top_p.is_none());
    }

    #[test]
    fn new_has_no_top_k() {
        let params = SamplingParameters::new();
        assert!(params.top_k.is_none());
    }

    #[test]
    fn with_temperature() {
        let params = SamplingParameters::new().with_temperature(0.7);
        assert_eq!(params.temperature, Some(0.7));
    }

    #[test]
    fn with_top_p() {
        let params = SamplingParameters::new().with_top_p(0.9);
        assert_eq!(params.top_p, Some(0.9));
    }

    #[test]
    fn with_top_k() {
        let params = SamplingParameters::new().with_top_k(40);
        assert_eq!(params.top_k, Some(40));
    }

    #[test]
    fn default_same_as_new() {
        let params1 = SamplingParameters::new();
        let params2 = SamplingParameters::default();
        assert_eq!(params1.temperature, params2.temperature);
        assert_eq!(params1.top_p, params2.top_p);
        assert_eq!(params1.top_k, params2.top_k);
    }

    #[test]
    fn builder_chain() {
        let params = SamplingParameters::new()
            .with_temperature(0.5)
            .with_top_p(0.8)
            .with_top_k(50);

        assert_eq!(params.temperature, Some(0.5));
        assert_eq!(params.top_p, Some(0.8));
        assert_eq!(params.top_k, Some(50));
    }

    #[test]
    fn is_serializable() {
        let params = SamplingParameters::new().with_temperature(0.5);
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("0.5"));
    }

    #[test]
    fn is_debug() {
        let params = SamplingParameters::new();
        let debug = format!("{:?}", params);
        assert!(debug.contains("SamplingParameters"));
    }
}

mod token_usage_tests {
    use super::*;

    #[test]
    fn new_sets_input() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.input, 100);
    }

    #[test]
    fn new_sets_output() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.output, 50);
    }

    #[test]
    fn new_calculates_total() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.total, 150);
    }

    #[test]
    fn new_has_no_cache_read() {
        let usage = TokenUsage::new(100, 50);
        assert!(usage.cache_read.is_none());
    }

    #[test]
    fn new_has_no_cache_creation() {
        let usage = TokenUsage::new(100, 50);
        assert!(usage.cache_creation.is_none());
    }

    #[test]
    fn with_cache_read() {
        let usage = TokenUsage::new(100, 50).with_cache_read(20);
        assert_eq!(usage.cache_read, Some(20));
    }

    #[test]
    fn with_cache_creation() {
        let usage = TokenUsage::new(100, 50).with_cache_creation(10);
        assert_eq!(usage.cache_creation, Some(10));
    }

    #[test]
    fn is_serializable() {
        let usage = TokenUsage::new(100, 50);
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("input_tokens"));
        assert!(json.contains("output_tokens"));
        assert!(json.contains("total_tokens"));
    }

    #[test]
    fn is_debug() {
        let usage = TokenUsage::new(100, 50);
        let debug = format!("{:?}", usage);
        assert!(debug.contains("TokenUsage"));
    }
}
