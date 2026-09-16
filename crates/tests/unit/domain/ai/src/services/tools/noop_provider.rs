//! Tests for NoopToolProvider.

use serde_json::json;
use systemprompt_ai::services::tools::NoopToolProvider;
use systemprompt_identifiers::{AgentName, McpServerId};
use systemprompt_test_fixtures::fixture_actor;
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider};

mod noop_provider_tests {
    use super::*;

    fn create_context() -> ToolContext {
        ToolContext::new(fixture_actor(), "test-token")
    }

    #[tokio::test]
    async fn list_tools_returns_empty_vec() {
        let provider = NoopToolProvider::new();
        let context = create_context();

        let tools = provider
            .list_tools(
                &AgentName::try_new("agent").expect("valid AgentName"),
                &context,
            )
            .await
            .unwrap();

        assert!(tools.tools.is_empty());
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

        let result = provider
            .call_tool(
                &request,
                &McpServerId::try_new("service").expect("valid McpServerId"),
                &context,
            )
            .await;

        let error = result.unwrap_err();
        assert!(error.to_string().contains("NoopToolProvider"));
        assert!(error.to_string().contains("some_tool"));
    }

    #[tokio::test]
    async fn refresh_connections_succeeds() {
        let provider = NoopToolProvider::new();

        let result = provider
            .refresh_connections(&AgentName::try_new("agent").expect("valid AgentName"))
            .await;

        result.expect("should succeed");
    }

    #[tokio::test]
    async fn health_check_returns_empty_map() {
        let provider = NoopToolProvider::new();

        let health = provider.health_check().await.unwrap();

        assert!(health.is_empty());
    }
}
