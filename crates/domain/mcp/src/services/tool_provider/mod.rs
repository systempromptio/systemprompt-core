mod context;
mod conversions;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult, ToolContext, ToolDefinition, ToolProvider, ToolProviderError,
    ToolProviderResult,
};

use crate::services::client::{validate_connection, McpClient};
use crate::services::registry::RegistryManager;

use context::{create_request_context, load_agent_servers};
use conversions::{to_tool_definition, to_tool_result};

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
}

#[async_trait]
impl ToolProvider for McpToolProvider {
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>> {
        let assigned_servers = load_agent_servers(agent_name).map_err(|e| {
            ToolProviderError::ConfigurationError(format!("Failed to load agent config: {e}"))
        })?;

        info!(
            agent = agent_name,
            servers = %assigned_servers.join(", "),
            "Listing tools for agent from MCP servers"
        );

        let request_ctx = create_request_context(context)?;
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
                        all_tools.push(to_tool_definition(&tool));
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
        let request_ctx = create_request_context(context)?;

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

        Ok(to_tool_result(&result))
    }

    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()> {
        let assigned_servers = load_agent_servers(agent_name).map_err(|e| {
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
            .map_err(|e| ToolProviderError::Internal(format!("Failed to get configuration: {e}")))?
            .api_server_url
            .clone();

        for server_name in assigned_servers {
            validate_server_connection(&server_name, &api_server_url).await;
        }

        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        let mut health_status = HashMap::new();

        let config_api_server_url = systemprompt_models::Config::get()
            .map_err(|e| ToolProviderError::Internal(format!("Failed to get configuration: {e}")))?
            .api_server_url
            .clone();

        if let Ok(servers) = RegistryManager::get_enabled_servers() {
            for server in servers {
                let is_healthy =
                    check_server_health(&server.name, server.port, &config_api_server_url).await;
                health_status.insert(server.name, is_healthy);
            }
        }

        Ok(health_status)
    }
}

async fn validate_server_connection(server_name: &str, api_server_url: &str) {
    if let Ok(Some(server_config)) = RegistryManager::find_server(server_name) {
        let url = server_config.endpoint(api_server_url);
        if let Ok(parsed_url) = url::Url::parse(&url) {
            let host = parsed_url.host_str().unwrap_or("127.0.0.1");
            let port = parsed_url.port().unwrap_or(80);

            match validate_connection(server_name, host, port).await {
                Ok(result) if result.success => {
                    info!(server = server_name, "MCP server connection validated");
                },
                Ok(result) => {
                    warn!(
                        server = server_name,
                        error = result.error_message.as_deref().unwrap_or("[no error]"),
                        "MCP server connection validation failed"
                    );
                },
                Err(e) => {
                    warn!(
                        server = server_name,
                        error = %e,
                        "Failed to validate MCP server connection"
                    );
                },
            }
        }
    }
}

async fn check_server_health(server_name: &str, server_port: u16, api_server_url: &str) -> bool {
    let url = format!("{}/api/v1/mcp/{}/mcp", api_server_url, server_name);

    let Ok(parsed_url) = url::Url::parse(&url) else {
        return false;
    };

    let host = parsed_url.host_str().unwrap_or("127.0.0.1");
    let actual_port = if server_port > 0 {
        server_port
    } else {
        parsed_url.port().unwrap_or(80)
    };

    validate_connection(server_name, host, actual_port)
        .await
        .map(|r| r.success)
        .unwrap_or(false)
}
