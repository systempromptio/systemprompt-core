use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, warn};

use systemprompt_core_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult, ToolContent, ToolContext, ToolDefinition, ToolProvider,
    ToolProviderError, ToolProviderResult,
};

use crate::services::client::{validate_connection, McpClient};
use crate::services::deployment::DeploymentService;
use crate::services::registry::RegistryManager;

#[derive(Debug, Clone)]
pub struct McpToolProvider {
    db_pool: DbPool,
}

impl McpToolProvider {
    pub fn new(app_context: &Arc<AppContext>) -> Self {
        let db_pool = Arc::clone(app_context.db_pool());
        Self { db_pool }
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    fn to_tool_definition(mcp_tool: &systemprompt_models::ai::tools::McpTool) -> ToolDefinition {
        ToolDefinition {
            name: mcp_tool.name.clone(),
            description: mcp_tool.description.clone(),
            input_schema: mcp_tool.input_schema.clone(),
            output_schema: mcp_tool.output_schema.clone(),
            service_id: mcp_tool.service_id.to_string(),
            terminal_on_success: mcp_tool.terminal_on_success,
            model_config: mcp_tool
                .model_config
                .as_ref()
                .and_then(|c| serde_json::to_value(c).ok()),
        }
    }

    fn to_tool_result(rmcp_result: &rmcp::model::CallToolResult) -> ToolCallResult {
        let content = rmcp_result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(text) => Some(ToolContent::Text {
                    text: text.text.clone(),
                }),
                rmcp::model::RawContent::Image(img) => Some(ToolContent::Image {
                    data: img.data.clone(),
                    mime_type: img.mime_type.clone(),
                }),
                rmcp::model::RawContent::ResourceLink(res) => Some(ToolContent::Resource {
                    uri: res.uri.clone(),
                    mime_type: res.mime_type.clone(),
                }),
                _ => None,
            })
            .collect();

        ToolCallResult {
            content,
            structured_content: rmcp_result.structured_content.clone(),
            is_error: rmcp_result.is_error,
            meta: rmcp_result
                .meta
                .as_ref()
                .and_then(|m| serde_json::to_value(m).ok()),
        }
    }

    fn create_request_context(ctx: &ToolContext) -> systemprompt_models::RequestContext {
        use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};

        let session_id = ctx
            .session_id
            .as_ref()
            .map_or_else(SessionId::system, |s| SessionId::new(s.clone()));
        let trace_id = ctx
            .trace_id
            .as_ref()
            .map_or_else(TraceId::generate, |t| TraceId::new(t.clone()));
        let context_id = ContextId::generate();
        let agent_name = AgentName::system();

        let mut request_ctx =
            systemprompt_models::RequestContext::new(session_id, trace_id, context_id, agent_name)
                .with_auth_token(ctx.auth_token.clone());

        if let Some(ai_tool_call_id) = &ctx.ai_tool_call_id {
            request_ctx = request_ctx.with_ai_tool_call_id(ai_tool_call_id.clone().into());
        }

        request_ctx
    }

    fn load_agent_servers(agent_name: &str) -> Result<Vec<String>> {
        let config = DeploymentService::load_config()?;
        let agent_name_type = systemprompt_identifiers::AgentName::new(agent_name);

        let agent = config
            .agents
            .get(agent_name_type.as_str())
            .ok_or_else(|| anyhow::anyhow!("Agent '{agent_name}' not found in services.yaml"))?;

        Ok(agent.metadata.mcp_servers.clone())
    }
}

#[async_trait]
impl ToolProvider for McpToolProvider {
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>> {
        let assigned_servers = Self::load_agent_servers(agent_name).map_err(|e| {
            ToolProviderError::ConfigurationError(format!("Failed to load agent config: {e}"))
        })?;

        info!(
            agent = agent_name,
            servers = %assigned_servers.join(", "),
            "Listing tools for agent from MCP servers"
        );

        let request_ctx = Self::create_request_context(context);
        let mut all_tools = Vec::new();

        for server_name in &assigned_servers {
            match McpClient::list_tools(server_name, &request_ctx).await {
                Ok(tools) => {
                    info!(
                        server = server_name,
                        tool_count = tools.len(),
                        "Loaded tools from MCP server"
                    );
                    for tool in tools {
                        all_tools.push(Self::to_tool_definition(&tool));
                    }
                },
                Err(e) => {
                    warn!(
                        server = server_name,
                        error = %e,
                        "Failed to list tools from MCP server"
                    );
                },
            }
        }

        info!(
            agent = agent_name,
            total_tools = all_tools.len(),
            "Total tools loaded for agent"
        );

        Ok(all_tools)
    }

    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        service_id: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult> {
        let request_ctx = Self::create_request_context(context);

        info!(
            tool = &request.name,
            service = service_id,
            "Executing tool via MCP"
        );

        let result = McpClient::call_tool(
            service_id,
            request.name.clone(),
            Some(request.arguments.clone()),
            &request_ctx,
            &self.db_pool,
        )
        .await
        .map_err(|e| ToolProviderError::ExecutionFailed(e.to_string()))?;

        Ok(Self::to_tool_result(&result))
    }

    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()> {
        let assigned_servers = Self::load_agent_servers(agent_name).map_err(|e| {
            ToolProviderError::ConfigurationError(format!("Failed to load agent config: {e}"))
        })?;

        info!(
            agent = agent_name,
            servers = %assigned_servers.join(", "),
            "Refreshing MCP connections for agent"
        );

        RegistryManager::validate().map_err(|e| {
            ToolProviderError::Internal(format!("Failed to validate registry: {e}"))
        })?;

        let api_server_url = systemprompt_models::Config::get()
            .map(|c| c.api_server_url.clone())
            .unwrap_or_default();
        for server_name in assigned_servers {
            if let Ok(Some(server_config)) = RegistryManager::find_server(&server_name) {
                let url = server_config.endpoint(&api_server_url);
                if let Ok(parsed_url) = url::Url::parse(&url) {
                    let host = parsed_url.host_str().unwrap_or("127.0.0.1");
                    let port = parsed_url.port().unwrap_or(80);

                    match validate_connection(&server_name, host, port).await {
                        Ok(result) if result.success => {
                            info!(server = &server_name, "MCP server connection validated");
                        },
                        Ok(result) => {
                            warn!(
                                server = &server_name,
                                error = result.error_message.unwrap_or_default(),
                                "MCP server connection validation failed"
                            );
                        },
                        Err(e) => {
                            warn!(
                                server = &server_name,
                                error = %e,
                                "Failed to validate MCP server connection"
                            );
                        },
                    }
                }
            }
        }

        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        use crate::services::registry::RegistryManager;

        let mut health_status = HashMap::new();

        let config_api_server_url = systemprompt_models::Config::get()
            .map(|c| c.api_server_url.clone())
            .unwrap_or_default();
        if let Ok(servers) = RegistryManager::get_enabled_servers() {
            for server in servers {
                let api_server_url = &config_api_server_url;
                let url = format!("{}/api/v1/mcp/{}/mcp", api_server_url, server.name);

                let is_healthy = if let Ok(parsed_url) = url::Url::parse(&url) {
                    let host = parsed_url.host_str().unwrap_or("127.0.0.1");
                    let actual_port = if server.port > 0 {
                        server.port
                    } else {
                        parsed_url.port().unwrap_or(80)
                    };

                    validate_connection(&server.name, host, actual_port)
                        .await
                        .map(|r| r.success)
                        .unwrap_or(false)
                } else {
                    false
                };

                health_status.insert(server.name, is_healthy);
            }
        }

        Ok(health_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_content_text() {
        let content = ToolContent::text("Hello, world!");
        match content {
            ToolContent::Text { text } => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolCallResult::success("Operation completed");
        assert_eq!(result.is_error, Some(false));
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolCallResult::error("Something went wrong");
        assert_eq!(result.is_error, Some(true));
    }
}
