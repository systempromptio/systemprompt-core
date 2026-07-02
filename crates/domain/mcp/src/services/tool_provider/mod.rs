//! `ToolProvider` implementation backed by MCP servers.
//!
//! Resolves an agent's assigned servers, lists their tools, and routes tool
//! calls through per-server resilience guards (circuit breaker and bulkhead).

mod context;
pub mod conversions;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use tracing::{info, warn};

use systemprompt_database::DbPool;
use systemprompt_database::resilience::{ResilienceConfig, ResilienceError, ResilienceGuard};
use systemprompt_identifiers::McpServerId;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult, ToolContext, ToolDefinition, ToolProvider, ToolProviderError,
    ToolProviderResult,
};

use crate::error::McpDomainError;
use crate::services::client::{
    McpClient, rewrite_url_for_internal_use, validate_connection, validate_connection_by_url,
};
pub use crate::services::registry::RegistryService;

use context::{create_request_context, load_agent_servers};
use conversions::{to_tool_definition, to_tool_result};

fn map_resilience_err(err: ResilienceError<McpDomainError>, server: &str) -> ToolProviderError {
    match err {
        ResilienceError::Inner(inner) => ToolProviderError::ExecutionFailed(inner.to_string()),
        ResilienceError::CircuitOpen { .. } => ToolProviderError::ExecutionFailed(format!(
            "circuit breaker open for MCP server {server}; failing fast"
        )),
        ResilienceError::BulkheadFull { .. } => ToolProviderError::ExecutionFailed(format!(
            "MCP server {server} unavailable: concurrency limit reached"
        )),
        ResilienceError::Timeout { after } => ToolProviderError::ExecutionFailed(format!(
            "MCP server {server} timed out after {after:?}"
        )),
    }
}

type GuardMap = Arc<Mutex<HashMap<String, Arc<ResilienceGuard>>>>;

#[derive(Debug, Clone)]
pub struct McpToolProvider {
    db_pool: DbPool,
    registry: RegistryService,
    resilience: ResilienceSettings,
    guards: GuardMap,
}

impl McpToolProvider {
    pub fn new(
        db_pool: DbPool,
        registry: RegistryService,
        resilience: &ResilienceSettings,
    ) -> Self {
        Self {
            db_pool,
            registry,
            resilience: *resilience,
            guards: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    fn guard_for(&self, server: &str) -> Arc<ResilienceGuard> {
        let mut guards = self.guards.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(existing) = guards.get(server) {
            return Arc::clone(existing);
        }
        let guard = Arc::new(ResilienceGuard::new(
            server,
            ResilienceConfig::from(&self.resilience),
        ));
        guards.insert(server.to_owned(), Arc::clone(&guard));
        guard
    }
}

#[async_trait]
impl ToolProvider for McpToolProvider {
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>> {
        let assigned_servers =
            load_agent_servers(agent_name).map_err(|e| ToolProviderError::ConfigurationError {
                message: format!("Failed to load agent config: {e}"),
            })?;

        info!(
            agent = agent_name,
            servers = %assigned_servers.join(", "),
            "Listing tools for agent from MCP servers"
        );

        let mut all_tools = Vec::new();

        for server_name in &assigned_servers {
            let server_config = self.registry.get_server(server_name).map_err(|e| {
                ToolProviderError::ConfigurationError {
                    message: format!("Failed to resolve MCP server {server_name}: {e}"),
                }
            })?;
            let request_ctx = create_request_context(context, &server_config)?;
            match McpClient::list_tools(&server_config, &request_ctx).await {
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
        service_id: &McpServerId,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult> {
        let server_config = self.registry.get_server(service_id.as_str()).map_err(|e| {
            ToolProviderError::ConfigurationError {
                message: format!("Failed to resolve MCP server {service_id}: {e}"),
            }
        })?;
        let request_ctx = create_request_context(context, &server_config)?;

        info!(
            tool = &request.name,
            service = service_id.as_str(),
            "Executing tool via MCP"
        );

        let guard = self.guard_for(service_id.as_str());
        let result = guard
            .execute(McpDomainError::classify, || {
                McpClient::call_tool(
                    &server_config,
                    request.name.clone(),
                    Some(request.arguments.clone()),
                    &request_ctx,
                )
            })
            .await
            .map_err(|err| map_resilience_err(err, service_id.as_str()))?;

        Ok(to_tool_result(&result))
    }

    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()> {
        let assigned_servers =
            load_agent_servers(agent_name).map_err(|e| ToolProviderError::ConfigurationError {
                message: format!("Failed to load agent config: {e}"),
            })?;

        info!(
            agent = agent_name,
            servers = %assigned_servers.join(", "),
            "Refreshing MCP connections for agent"
        );

        self.registry.validate().map_err(|e| {
            ToolProviderError::Internal(format!("Failed to validate registry: {e}"))
        })?;

        let api_server_url = systemprompt_models::Config::get()
            .map_err(|e| ToolProviderError::Config {
                message: "Failed to get configuration".to_owned(),
                source: Box::new(e),
            })?
            .api_server_url
            .clone();

        for server_name in assigned_servers {
            validate_server_connection(&self.registry, &server_name, &api_server_url).await;
        }

        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        let mut health_status = HashMap::new();

        let config_api_server_url = systemprompt_models::Config::get()
            .map_err(|e| ToolProviderError::Config {
                message: "Failed to get configuration".to_owned(),
                source: Box::new(e),
            })?
            .api_server_url
            .clone();

        if let Ok(servers) = self.registry.get_managed_servers() {
            for server in servers {
                let is_healthy =
                    check_server_health(&server.name, server.port, &config_api_server_url).await;
                let breaker = self.guard_for(&server.name);
                if is_healthy {
                    breaker.breaker().record_success();
                } else {
                    breaker.breaker().record_failure();
                }
                health_status.insert(server.name, is_healthy);
            }
        }

        Ok(health_status)
    }
}

async fn validate_server_connection(
    registry: &RegistryService,
    server_name: &str,
    api_server_url: &str,
) {
    if let Ok(Some(server_config)) = registry.find_server(server_name) {
        let host = &server_config.host;
        let port = server_config.port;

        let result = if port == 0 {
            let url = server_config.endpoint(api_server_url);
            let url = rewrite_url_for_internal_use(&url);
            validate_connection_by_url(server_name, &url).await
        } else {
            validate_connection(server_name, host, port).await
        };

        match result {
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
        .is_ok_and(|r| r.success)
}
