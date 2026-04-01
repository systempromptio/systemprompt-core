//! Tests for ChatRequest, ChatResponse, LlmProviderError, and ToolExecutionContext.

use systemprompt_identifiers::{SessionId, TraceId};
use systemprompt_provider_contracts::{
    ChatMessage, ChatRequest, ChatResponse, LlmProviderError, SamplingParameters,
    TokenUsage, ToolExecutionContext,
};

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
        req.sampling.as_ref().expect("sampling should be set");
    }

    #[test]
    fn with_tools() {
        use systemprompt_provider_contracts::ToolDefinition;
        let tools = vec![ToolDefinition::new("tool", "svc")];
        let req = test_request().with_tools(tools);
        assert_eq!(req.tools.expect("tools should be set").len(), 1);
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
        resp.usage.as_ref().expect("usage should be set");
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
