//! Tests for ToolContext and ToolProviderError.

use systemprompt_provider_contracts::{ToolContext, ToolProviderError};

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
