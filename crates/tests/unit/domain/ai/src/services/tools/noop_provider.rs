//! Tests for NoopToolProvider.

use serde_json::json;
use systemprompt_ai::services::tools::NoopToolProvider;
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider};

mod noop_provider_tests {
    use super::*;

    fn create_context() -> ToolContext {
        ToolContext::new("test-token")
    }

    #[tokio::test]
    async fn list_tools_returns_empty_vec() {
        let provider = NoopToolProvider::new();
        let context = create_context();

        let tools = provider.list_tools("agent", &context).await.unwrap();

        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn call_tool_returns_error() {
        let provider = NoopToolProvider::new();
        let context = create_context();
        let request = ToolCallRequest {
            tool_call_id: "call-123".to_string(),
            name: "some_tool".to_string(),
            arguments: json!({}),
        };

        let result = provider.call_tool(&request, "service", &context).await;

        let error = result.unwrap_err();
        assert!(error.to_string().contains("NoopToolProvider"));
        assert!(error.to_string().contains("some_tool"));
    }

    #[tokio::test]
    async fn refresh_connections_succeeds() {
        let provider = NoopToolProvider::new();

        let result = provider.refresh_connections("agent").await;

        result.expect("should succeed");
    }

    #[tokio::test]
    async fn health_check_returns_empty_map() {
        let provider = NoopToolProvider::new();

        let health = provider.health_check().await.unwrap();

        assert!(health.is_empty());
    }

    #[test]
    fn is_debug() {
        let provider = NoopToolProvider::new();
        let debug = format!("{:?}", provider);
        assert!(debug.contains("NoopToolProvider"));
    }
}
