//! Tests for LLM provider types

use systemprompt_identifiers::{SessionId, TraceId};
use systemprompt_provider_contracts::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, LlmProviderError, SamplingParameters,
    TokenUsage, ToolExecutionContext,
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
    fn deserializes_system() {
        let role: ChatRole = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(role, ChatRole::System);
    }

    #[test]
    fn deserializes_user() {
        let role: ChatRole = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(role, ChatRole::User);
    }

    #[test]
    fn is_copy() {
        let role = ChatRole::User;
        let copied: ChatRole = role;
        assert_eq!(role, copied);
    }

    #[test]
    fn is_eq() {
        assert_eq!(ChatRole::User, ChatRole::User);
        assert_ne!(ChatRole::User, ChatRole::System);
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
    fn is_deserializable() {
        let json = r#"{"role":"user","content":"hi"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, ChatRole::User);
        assert_eq!(msg.content, "hi");
    }

    #[test]
    fn is_clone() {
        let msg = ChatMessage::user("test");
        let cloned = msg.clone();
        assert_eq!(cloned.content, msg.content);
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
    fn is_copy() {
        let params = SamplingParameters::new().with_temperature(0.5);
        let copied: SamplingParameters = params;
        assert_eq!(params.temperature, copied.temperature);
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
    fn is_deserializable() {
        let json = r#"{"input_tokens":100,"output_tokens":50,"total_tokens":150}"#;
        let usage: TokenUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.input, 100);
        assert_eq!(usage.output, 50);
    }

    #[test]
    fn is_copy() {
        let usage = TokenUsage::new(100, 50);
        let copied: TokenUsage = usage;
        assert_eq!(usage.total, copied.total);
    }

    #[test]
    fn is_debug() {
        let usage = TokenUsage::new(100, 50);
        let debug = format!("{:?}", usage);
        assert!(debug.contains("TokenUsage"));
    }
}

mod chat_request_tests {
    use super::*;

    fn test_request() -> ChatRequest {
        ChatRequest::new(vec![ChatMessage::user("Hi")], "gpt-4", 1000)
    }

    #[test]
    fn new_sets_messages() {
        let req = test_request();
        assert_eq!(req.messages.len(), 1);
    }

    #[test]
    fn new_sets_model() {
        let req = test_request();
        assert_eq!(req.model, "gpt-4");
    }

    #[test]
    fn new_sets_max_output_tokens() {
        let req = test_request();
        assert_eq!(req.max_output_tokens, 1000);
    }

    #[test]
    fn new_has_no_sampling() {
        let req = test_request();
        assert!(req.sampling.is_none());
    }

    #[test]
    fn new_has_no_tools() {
        let req = test_request();
        assert!(req.tools.is_none());
    }

    #[test]
    fn new_has_no_response_schema() {
        let req = test_request();
        assert!(req.response_schema.is_none());
    }

    #[test]
    fn with_sampling() {
        let sampling = SamplingParameters::new().with_temperature(0.5);
        let req = test_request().with_sampling(sampling);
        assert!(req.sampling.is_some());
    }

    #[test]
    fn with_tools() {
        use systemprompt_provider_contracts::ToolDefinition;
        let tools = vec![ToolDefinition::new("tool", "svc")];
        let req = test_request().with_tools(tools);
        assert!(req.tools.is_some());
        assert_eq!(req.tools.unwrap().len(), 1);
    }

    #[test]
    fn with_response_schema() {
        let schema = serde_json::json!({"type": "object"});
        let req = test_request().with_response_schema(schema.clone());
        assert_eq!(req.response_schema, Some(schema));
    }

    #[test]
    fn is_clone() {
        let req = test_request();
        let cloned = req.clone();
        assert_eq!(cloned.model, req.model);
    }

    #[test]
    fn is_debug() {
        let req = test_request();
        let debug = format!("{:?}", req);
        assert!(debug.contains("ChatRequest"));
    }
}

mod chat_response_tests {
    use super::*;

    fn test_response() -> ChatResponse {
        ChatResponse::new("Hello!", "gpt-4")
    }

    #[test]
    fn new_sets_content() {
        let resp = test_response();
        assert_eq!(resp.content, "Hello!");
    }

    #[test]
    fn new_sets_model() {
        let resp = test_response();
        assert_eq!(resp.model, "gpt-4");
    }

    #[test]
    fn new_has_empty_tool_calls() {
        let resp = test_response();
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn new_has_no_usage() {
        let resp = test_response();
        assert!(resp.usage.is_none());
    }

    #[test]
    fn new_has_zero_latency() {
        let resp = test_response();
        assert_eq!(resp.latency_ms, 0);
    }

    #[test]
    fn with_tool_calls() {
        use systemprompt_provider_contracts::ToolCallRequest;
        let calls = vec![ToolCallRequest {
            tool_call_id: "1".to_string(),
            name: "tool".to_string(),
            arguments: serde_json::json!({}),
        }];
        let resp = test_response().with_tool_calls(calls);
        assert_eq!(resp.tool_calls.len(), 1);
    }

    #[test]
    fn with_usage() {
        let usage = TokenUsage::new(100, 50);
        let resp = test_response().with_usage(usage);
        assert!(resp.usage.is_some());
    }

    #[test]
    fn with_latency() {
        let resp = test_response().with_latency(500);
        assert_eq!(resp.latency_ms, 500);
    }

    #[test]
    fn is_serializable() {
        let resp = test_response();
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("Hello!"));
    }

    #[test]
    fn is_clone() {
        let resp = test_response();
        let cloned = resp.clone();
        assert_eq!(cloned.content, resp.content);
    }

    #[test]
    fn is_debug() {
        let resp = test_response();
        let debug = format!("{:?}", resp);
        assert!(debug.contains("ChatResponse"));
    }
}

mod llm_provider_error_tests {
    use super::*;

    #[test]
    fn model_not_supported_contains_name() {
        let err = LlmProviderError::ModelNotSupported("gpt-5".to_string());
        assert!(err.to_string().contains("gpt-5"));
    }

    #[test]
    fn provider_not_available_contains_name() {
        let err = LlmProviderError::ProviderNotAvailable("openai".to_string());
        assert!(err.to_string().contains("openai"));
    }

    #[test]
    fn rate_limit_exceeded_message() {
        let err = LlmProviderError::RateLimitExceeded;
        let msg = err.to_string().to_lowercase();
        assert!(msg.contains("rate") || msg.contains("limit"));
    }

    #[test]
    fn authentication_failed_contains_message() {
        let err = LlmProviderError::AuthenticationFailed("invalid key".to_string());
        assert!(err.to_string().contains("invalid key"));
    }

    #[test]
    fn invalid_request_contains_message() {
        let err = LlmProviderError::InvalidRequest("bad format".to_string());
        assert!(err.to_string().contains("bad format"));
    }

    #[test]
    fn generation_failed_contains_message() {
        let err = LlmProviderError::GenerationFailed("timeout".to_string());
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(LlmProviderError::RateLimitExceeded);
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn is_debug() {
        let err = LlmProviderError::RateLimitExceeded;
        let debug = format!("{:?}", err);
        assert!(debug.contains("RateLimitExceeded"));
    }
}

mod tool_execution_context_tests {
    use super::*;

    #[test]
    fn new_sets_auth_token() {
        let ctx = ToolExecutionContext::new("token123");
        assert_eq!(ctx.auth_token, "token123");
    }

    #[test]
    fn new_has_no_session_id() {
        let ctx = ToolExecutionContext::new("token");
        assert!(ctx.session_id.is_none());
    }

    #[test]
    fn new_has_no_trace_id() {
        let ctx = ToolExecutionContext::new("token");
        assert!(ctx.trace_id.is_none());
    }

    #[test]
    fn new_has_no_model_overrides() {
        let ctx = ToolExecutionContext::new("token");
        assert!(ctx.model_overrides.is_none());
    }

    #[test]
    fn with_session_id() {
        let session_id = SessionId::new("sess-1");
        let ctx = ToolExecutionContext::new("token").with_session_id(session_id.clone());
        assert_eq!(ctx.session_id, Some(session_id));
    }

    #[test]
    fn with_trace_id() {
        let trace_id = TraceId::new("trace-1");
        let ctx = ToolExecutionContext::new("token").with_trace_id(trace_id.clone());
        assert_eq!(ctx.trace_id, Some(trace_id));
    }

    #[test]
    fn with_model_overrides() {
        let overrides = serde_json::json!({"temperature": 0.5});
        let ctx = ToolExecutionContext::new("token").with_model_overrides(overrides.clone());
        assert_eq!(ctx.model_overrides, Some(overrides));
    }

    #[test]
    fn is_clone() {
        let ctx = ToolExecutionContext::new("token");
        let cloned = ctx.clone();
        assert_eq!(cloned.auth_token, ctx.auth_token);
    }

    #[test]
    fn is_debug() {
        let ctx = ToolExecutionContext::new("token");
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("ToolExecutionContext"));
    }
}
