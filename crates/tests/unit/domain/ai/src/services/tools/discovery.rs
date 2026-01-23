use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use systemprompt_ai::services::tools::ToolDiscovery;
use systemprompt_identifiers::AgentName;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult, ToolContext, ToolDefinition, ToolProvider,
    ToolProviderError, ToolProviderResult,
};
use serde_json::json;

struct MockToolProvider {
    tools: Vec<ToolDefinition>,
    refresh_called: std::sync::atomic::AtomicBool,
}

impl MockToolProvider {
    fn new(tools: Vec<ToolDefinition>) -> Self {
        Self {
            tools,
            refresh_called: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn was_refresh_called(&self) -> bool {
        self.refresh_called.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl ToolProvider for MockToolProvider {
    async fn list_tools(
        &self,
        _agent_name: &str,
        _context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>> {
        Ok(self.tools.clone())
    }

    async fn call_tool(
        &self,
        _request: &ToolCallRequest,
        _service_id: &str,
        _context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult> {
        Ok(ToolCallResult {
            content: vec![],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    async fn refresh_connections(&self, _agent_name: &str) -> ToolProviderResult<()> {
        self.refresh_called.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        Ok(HashMap::new())
    }

    async fn find_tool(
        &self,
        _agent_name: &str,
        tool_name: &str,
        _context: &ToolContext,
    ) -> ToolProviderResult<Option<ToolDefinition>> {
        Ok(self.tools.iter().find(|t| t.name == tool_name).cloned())
    }
}

fn create_test_tool(name: &str, description: &str) -> ToolDefinition {
    ToolDefinition::new(name, "test-service")
        .with_description(description)
        .with_input_schema(json!({"type": "object", "properties": {}}))
}

fn create_test_context() -> ToolContext {
    ToolContext::new("test-token")
}

mod tool_discovery_tests {
    use super::*;
    use systemprompt_models::execution::context::RequestContext;
    use systemprompt_identifiers::{SessionId, TraceId, ContextId};

    fn create_request_context() -> RequestContext {
        RequestContext::new(
            SessionId::new("test-session".to_string()),
            TraceId::new("test-trace".to_string()),
            ContextId::new("test-context".to_string()),
            AgentName::new("test-agent".to_string()),
        )
    }

    #[test]
    fn new_creates_discovery() {
        let provider = Arc::new(MockToolProvider::new(vec![]));
        let discovery = ToolDiscovery::new(provider);
        let debug_str = format!("{:?}", discovery);
        assert!(debug_str.contains("ToolDiscovery"));
    }

    #[tokio::test]
    async fn discover_tools_returns_empty_for_no_tools() {
        let provider = Arc::new(MockToolProvider::new(vec![]));
        let discovery = ToolDiscovery::new(provider);
        let agent_name = AgentName::new("test-agent".to_string());
        let context = create_request_context();

        let result = discovery.discover_tools(&agent_name, &context).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn discover_tools_returns_tools() {
        let tools = vec![
            create_test_tool("search", "Search the web"),
            create_test_tool("fetch", "Fetch a URL"),
        ];
        let provider = Arc::new(MockToolProvider::new(tools));
        let discovery = ToolDiscovery::new(provider);
        let agent_name = AgentName::new("test-agent".to_string());
        let context = create_request_context();

        let result = discovery.discover_tools(&agent_name, &context).await;

        assert!(result.is_ok());
        let mcp_tools = result.unwrap();
        assert_eq!(mcp_tools.len(), 2);
    }

    #[tokio::test]
    async fn discover_tools_calls_refresh() {
        let provider = Arc::new(MockToolProvider::new(vec![]));
        let discovery = ToolDiscovery::new(provider.clone());
        let agent_name = AgentName::new("test-agent".to_string());
        let context = create_request_context();

        let _ = discovery.discover_tools(&agent_name, &context).await;

        assert!(provider.was_refresh_called());
    }

    #[tokio::test]
    async fn find_tool_returns_none_for_missing_tool() {
        let provider = Arc::new(MockToolProvider::new(vec![]));
        let discovery = ToolDiscovery::new(provider);
        let agent_name = AgentName::new("test-agent".to_string());
        let context = create_request_context();

        let result = discovery
            .find_tool_for_agent(&agent_name, "nonexistent", &context)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn find_tool_returns_tool_when_found() {
        let tools = vec![create_test_tool("search", "Search the web")];
        let provider = Arc::new(MockToolProvider::new(tools));
        let discovery = ToolDiscovery::new(provider);
        let agent_name = AgentName::new("test-agent".to_string());
        let context = create_request_context();

        let result = discovery
            .find_tool_for_agent(&agent_name, "search", &context)
            .await;

        assert!(result.is_ok());
        let tool = result.unwrap();
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "search");
    }

    #[test]
    fn definitions_to_mcp_tools_converts_empty_list() {
        let definitions: Vec<ToolDefinition> = vec![];
        let mcp_tools = ToolDiscovery::definitions_to_mcp_tools(&definitions);
        assert!(mcp_tools.is_empty());
    }

    #[test]
    fn definitions_to_mcp_tools_converts_single_tool() {
        let definitions = vec![create_test_tool("test", "A test tool")];
        let mcp_tools = ToolDiscovery::definitions_to_mcp_tools(&definitions);

        assert_eq!(mcp_tools.len(), 1);
        assert_eq!(mcp_tools[0].name, "test");
        assert_eq!(mcp_tools[0].description, Some("A test tool".to_string()));
    }

    #[test]
    fn definitions_to_mcp_tools_converts_multiple_tools() {
        let definitions = vec![
            create_test_tool("search", "Search"),
            create_test_tool("fetch", "Fetch"),
            create_test_tool("analyze", "Analyze"),
        ];
        let mcp_tools = ToolDiscovery::definitions_to_mcp_tools(&definitions);

        assert_eq!(mcp_tools.len(), 3);
    }

    #[test]
    fn definitions_to_mcp_tools_preserves_schema() {
        let definition = ToolDefinition::new("test", "test-service")
            .with_description("Test")
            .with_input_schema(json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }))
            .with_output_schema(json!({"type": "string"}));

        let mcp_tools = ToolDiscovery::definitions_to_mcp_tools(&[definition]);

        assert_eq!(mcp_tools.len(), 1);
        let tool = &mcp_tools[0];
        assert!(tool.input_schema.as_ref().unwrap()["properties"]["query"].is_object());
    }
}
