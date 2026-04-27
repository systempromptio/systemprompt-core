use serde_json::json;
use systemprompt_ai::services::tools::NoopToolProvider;
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider, ToolProviderError};

mod noop_default_tests {
    use super::*;

    #[test]
    fn default_creates_same_as_new() {
        let from_new = NoopToolProvider::new();
        let from_default = NoopToolProvider::default();

        let debug_new = format!("{:?}", from_new);
        let debug_default = format!("{:?}", from_default);
        assert_eq!(debug_new, debug_default);
    }

    #[test]
    fn is_clone() {
        let provider = NoopToolProvider::new();
        let cloned = provider.clone();

        let debug_original = format!("{:?}", provider);
        let debug_cloned = format!("{:?}", cloned);
        assert_eq!(debug_original, debug_cloned);
    }

    #[test]
    fn is_copy() {
        let provider = NoopToolProvider::new();
        let copied = provider;
        let _still_valid = provider;
        let _ = format!("{:?}", copied);
    }
}

mod noop_error_details_tests {
    use super::*;

    #[tokio::test]
    async fn call_tool_error_contains_tool_name() {
        let provider = NoopToolProvider::new();
        let context = ToolContext::new("token");
        let request = ToolCallRequest {
            tool_call_id: "id-1".to_string(),
            name: "specific_tool_name".to_string(),
            arguments: json!({}),
        };

        let err = provider
            .call_tool(&request, "svc", &context)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("specific_tool_name"));
    }

    #[tokio::test]
    async fn call_tool_error_is_service_not_found() {
        let provider = NoopToolProvider::new();
        let context = ToolContext::new("token");
        let request = ToolCallRequest {
            tool_call_id: "id-2".to_string(),
            name: "any_tool".to_string(),
            arguments: json!({"key": "value"}),
        };

        let err = provider
            .call_tool(&request, "svc", &context)
            .await
            .unwrap_err();

        match err {
            ToolProviderError::ServiceNotFound(msg) => {
                assert!(msg.contains("NoopToolProvider"));
                assert!(msg.contains("any_tool"));
            },
            other => panic!("Expected ServiceNotFound, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn call_tool_ignores_service_id() {
        let provider = NoopToolProvider::new();
        let context = ToolContext::new("token");
        let request = ToolCallRequest {
            tool_call_id: "id-3".to_string(),
            name: "tool".to_string(),
            arguments: json!({}),
        };

        let err1 = provider
            .call_tool(&request, "service-a", &context)
            .await
            .unwrap_err();
        let err2 = provider
            .call_tool(&request, "service-b", &context)
            .await
            .unwrap_err();

        assert!(err1.to_string().contains("NoopToolProvider"));
        assert!(err2.to_string().contains("NoopToolProvider"));
    }

    #[tokio::test]
    async fn list_tools_ignores_agent_name() {
        let provider = NoopToolProvider::new();
        let context = ToolContext::new("token");

        let tools_a = provider.list_tools("agent-a", &context).await.unwrap();
        let tools_b = provider.list_tools("agent-b", &context).await.unwrap();

        assert!(tools_a.is_empty());
        assert!(tools_b.is_empty());
    }

    #[tokio::test]
    async fn refresh_connections_ignores_agent_name() {
        let provider = NoopToolProvider::new();

        let result_a = provider.refresh_connections("agent-x").await;
        let result_b = provider.refresh_connections("agent-y").await;

        assert!(result_a.is_ok());
        assert!(result_b.is_ok());
    }
}
