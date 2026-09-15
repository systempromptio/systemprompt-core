//! `ToolProvider` implementation backed by MCP servers.
//!
//! Resolves an agent's assigned servers, lists their tools, and routes tool
//! calls through per-server resilience guards (circuit breaker and bulkhead).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod context;
pub mod conversions;
mod health;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use tracing::{info, warn};

use systemprompt_database::DbPool;
use systemprompt_database::resilience::{ResilienceConfig, ResilienceError, ResilienceGuard};
use systemprompt_identifiers::{AgentName, McpServerId};
use systemprompt_models::services::ResilienceSettings;
use systemprompt_traits::{
    ServerListingFailure, ToolCallRequest, ToolCallResult, ToolContext, ToolInventory,
    ToolProvider, ToolProviderError, ToolProviderResult,
};

use crate::error::McpDomainError;
use crate::services::client::McpClient;
pub use crate::services::registry::RegistryService;

use context::{create_request_context, load_agent_servers};
use conversions::{to_tool_definition, to_tool_result};
use health::{check_server_connection, check_server_health};

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
        agent_name: &AgentName,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolInventory> {
        let assigned_servers =
            load_agent_servers(agent_name).map_err(|e| ToolProviderError::ConfigurationError {
                message: format!("Failed to load agent config: {e}"),
            })?;

        info!(
            agent = %agent_name,
            servers = %assigned_servers.join(", "),
            "Listing tools for agent from MCP servers"
        );

        let mut inventory = ToolInventory::default();

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
                    inventory.tools.extend(tools.iter().map(to_tool_definition));
                },
                Err(e) => {
                    warn!(
                        server = server_name,
                        error = %e,
                        "Failed to list tools from MCP server"
                    );
                    inventory.failed_servers.push(ServerListingFailure {
                        server: McpServerId::try_new(server_name.clone()).map_err(|e| {
                            ToolProviderError::ConfigurationError {
                                message: format!("invalid MCP server name {server_name}: {e}"),
                            }
                        })?,
                        message: e.to_string(),
                    });
                },
            }
        }

        info!(
            agent = %agent_name,
            total_tools = inventory.tools.len(),
            failed_servers = inventory.failed_servers.len(),
            "Tools loaded for agent"
        );

        Ok(inventory)
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

    async fn refresh_connections(&self, agent_name: &AgentName) -> ToolProviderResult<()> {
        let assigned_servers =
            load_agent_servers(agent_name).map_err(|e| ToolProviderError::ConfigurationError {
                message: format!("Failed to load agent config: {e}"),
            })?;

        info!(
            agent = %agent_name,
            servers = %assigned_servers.join(", "),
            "Refreshing MCP connections for agent"
        );

        self.registry.validate().map_err(|e| {
            ToolProviderError::Internal(format!("Failed to validate registry: {e}"))
        })?;

        let api_server_url = systemprompt_models::Config::get()
            .map_err(|e| ToolProviderError::ConfigurationError {
                message: format!("Failed to get configuration: {e}"),
            })?
            .api_server_url
            .clone();

        for server_name in assigned_servers {
            check_server_connection(&self.registry, &server_name, &api_server_url).await;
        }

        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        let mut health_status = HashMap::new();

        let config_api_server_url = systemprompt_models::Config::get()
            .map_err(|e| ToolProviderError::ConfigurationError {
                message: format!("Failed to get configuration: {e}"),
            })?
            .api_server_url
            .clone();

        let servers = self.registry.get_managed_servers().map_err(|e| {
            ToolProviderError::ConfigurationError {
                message: format!("Failed to list managed MCP servers: {e}"),
            }
        })?;
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

        Ok(health_status)
    }
}
