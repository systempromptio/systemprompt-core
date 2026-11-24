//! Tests for ToolContext and ToolProviderError.

use systemprompt_identifiers::{Actor, AiToolCallId, JwtToken, McpServerId, SessionId, TraceId};
use systemprompt_provider_contracts::{ToolContext, ToolProviderError};
use systemprompt_test_fixtures::unique_user_id;

mod tool_context_tests {
    use super::*;

    #[test]
    fn with_auth_token_sets_auth_token() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token123"));
        assert_eq!(
            ctx.auth_token.as_ref().map(JwtToken::as_str),
            Some("token123")
        );
    }

    #[test]
    fn new_has_no_auth_token() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")));
        assert!(ctx.auth_token.is_none());
    }

    #[test]
    fn new_defaults_session_id_to_none() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"));
        assert!(ctx.session_id.is_none());
    }

    #[test]
    fn new_defaults_trace_id_to_none() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"));
        assert!(ctx.trace_id.is_none());
    }

    #[test]
    fn new_defaults_ai_tool_call_id_to_none() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"));
        assert!(ctx.ai_tool_call_id.is_none());
    }

    #[test]
    fn new_defaults_headers_to_empty() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"));
        assert!(ctx.headers.is_empty());
    }

    #[test]
    fn with_session_id() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"))
            .with_session_id(SessionId::new("sess-1"));
        assert_eq!(ctx.session_id, Some(SessionId::new("sess-1")));
    }

    #[test]
    fn with_trace_id() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"))
            .with_trace_id(TraceId::new("trace-1"));
        assert_eq!(ctx.trace_id, Some(TraceId::new("trace-1")));
    }

    #[test]
    fn with_ai_tool_call_id() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"))
            .with_ai_tool_call_id(AiToolCallId::new("call-1"));
        assert_eq!(ctx.ai_tool_call_id, Some(AiToolCallId::new("call-1")));
    }

    #[test]
    fn with_header() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"))
            .with_header("X-Custom", "value");
        assert_eq!(ctx.headers.get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn multiple_headers() {
        let ctx = ToolContext::new(Actor::user(unique_user_id("context-and-error")))
            .with_auth_token(JwtToken::new("token"))
            .with_header("H1", "v1")
            .with_header("H2", "v2");
        assert_eq!(ctx.headers.len(), 2);
    }
}

mod tool_provider_error_tests {
    use super::*;

    #[test]
    fn timed_out_names_the_server() {
        let err = ToolProviderError::TimedOut {
            server: McpServerId::new("svc-1"),
            after: std::time::Duration::from_secs(3),
        };
        assert_eq!(
            err.to_string(),
            "Tool execution failed: MCP server svc-1 timed out after 3s"
        );
    }

    #[test]
    fn missing_caller_identity_names_the_header() {
        let err = ToolProviderError::MissingCallerIdentity;
        assert!(err.to_string().contains("x-user-id"));
    }

    #[test]
    fn configuration_error_contains_message() {
        let err = ToolProviderError::ConfigurationError {
            message: "missing url".to_string(),
        };
        assert!(err.to_string().contains("missing url"));
    }

    #[test]
    fn internal_contains_message() {
        let err = ToolProviderError::Internal("unknown".into());
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(ToolProviderError::NoTools {
            tool: "t".to_string(),
        });
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn is_debug() {
        let err = ToolProviderError::NoTools {
            tool: "t".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("NoTools"));
    }
}
