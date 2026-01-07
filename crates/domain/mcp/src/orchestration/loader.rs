use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use systemprompt_core_database::DbPool;
use systemprompt_models::ai::tools::McpTool;
use systemprompt_models::RequestContext;
use tracing::{debug, error};

use super::state::ServiceStateManager;
use crate::services::client::McpClient;
use crate::services::deployment::DeploymentService;
use crate::services::registry::RegistryManager;

#[derive(Debug, Clone)]
pub struct McpToolLoader {
    _db_pool: DbPool,
    service_manager: ServiceStateManager,
}

impl McpToolLoader {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            service_manager: ServiceStateManager::new(Arc::clone(&db_pool)),
            _db_pool: db_pool,
        }
    }

    pub async fn load_tools_for_servers(
        &self,
        server_names: &[String],
        context: &RequestContext,
    ) -> Result<HashMap<String, Vec<McpTool>>> {
        let deployment_config = DeploymentService::load_config()?;
        let user_permissions = extract_user_permissions(context)?;

        let mut tools_by_server = HashMap::new();
        let mut load_errors = Vec::new();
        let mut skipped_servers = Vec::new();

        for server_name in server_names {
            if !has_server_permission(&deployment_config, server_name, &user_permissions) {
                skipped_servers.push(server_name.clone());
                continue;
            }

            match load_with_timeout(self, server_name, context).await {
                Ok(tools) => {
                    tools_by_server.insert(server_name.clone(), tools);
                },
                Err(msg) => load_errors.push(msg),
            }
        }

        log_loading_summary(
            &skipped_servers,
            &load_errors,
            tools_by_server.len(),
            server_names.len(),
        );
        Ok(tools_by_server)
    }

    pub async fn load_server_tools(
        &self,
        server_name: &str,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        let mut retries = 0;
        let max_retries = 3;

        loop {
            match self.service_manager.get_mcp_service(server_name).await {
                Ok(Some(service)) => {
                    if service.status != "running" {
                        return Err(anyhow::anyhow!(
                            "MCP server '{}' is not running (status: {})",
                            server_name,
                            service.status
                        ));
                    }
                    return McpClient::list_tools(server_name, context).await;
                },
                Ok(None) => {
                    if retries < max_retries {
                        let backoff_ms = 100 * (2u64.pow(retries as u32));
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                        retries += 1;
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "MCP server '{}' not found in services database (after {} retries with \
                         {}ms DB lag tolerance)",
                        server_name,
                        max_retries,
                        100 * (2u64.pow(max_retries as u32) - 1)
                    ));
                },
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Database error querying MCP server '{}': {} (this indicates a database \
                         connectivity issue, not replication lag)",
                        server_name,
                        e
                    ));
                },
            }
        }
    }

    pub const fn service_manager(&self) -> &ServiceStateManager {
        &self.service_manager
    }
}

use super::McpServerMetadata;

impl McpToolLoader {
    pub async fn create_mcp_extensions(
        &self,
        server_names: &[String],
        base_url: &str,
        context: &RequestContext,
    ) -> Result<Vec<McpServerMetadata>> {
        if server_names.is_empty() {
            return Ok(vec![]);
        }

        let deployment_config = DeploymentService::load_config()?;
        let tools_by_server = self.load_tools_for_servers(server_names, context).await?;
        let mut servers_info = Vec::new();

        for server_name in server_names {
            if let Some(deployment) = deployment_config.mcp_servers.get(server_name) {
                let auth_value = if !deployment.oauth.required || deployment.oauth.scopes.is_empty()
                {
                    "anon".to_string()
                } else {
                    deployment
                        .oauth
                        .scopes
                        .first()
                        .map_or_else(|| "user".to_string(), ToString::to_string)
                };

                let runtime_status = self
                    .service_manager
                    .get_mcp_service(server_name)
                    .await?
                    .map_or_else(|| "not_started".to_string(), |s| s.status);

                let version = RegistryManager::find_server(server_name)
                    .ok()
                    .flatten()
                    .map(|s| s.version);

                let tools = tools_by_server.get(server_name).cloned();

                servers_info.push(McpServerMetadata {
                    name: server_name.clone(),
                    endpoint: format!("{}/api/v1/mcp/{}/mcp", base_url, server_name),
                    auth: auth_value,
                    status: runtime_status,
                    version,
                    tools,
                });
            } else {
                servers_info.push(McpServerMetadata {
                    name: server_name.clone(),
                    endpoint: format!("{}/api/v1/mcp/{}/mcp", base_url, server_name),
                    auth: "unknown".to_string(),
                    status: "not_in_config".to_string(),
                    version: None,
                    tools: None,
                });
            }
        }

        Ok(servers_info)
    }
}

fn extract_user_permissions(context: &RequestContext) -> Result<Vec<String>> {
    use systemprompt_core_oauth::services::validation::jwt::validate_jwt_token;

    let token = context.auth_token().as_str();
    if token.is_empty() {
        return Ok(vec![]);
    }

    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()
        .map_err(|e| anyhow::anyhow!("Failed to get JWT secret: {}", e))?;

    let config = systemprompt_models::Config::get()
        .map_err(|e| anyhow::anyhow!("Failed to get config: {}", e))?;

    let claims = validate_jwt_token(token, jwt_secret, &config.jwt_issuer, &config.jwt_audiences)
        .map_err(|e| {
        error!(error = %e, "JWT validation failed");
        anyhow::anyhow!("JWT validation failed: {}", e)
    })?;

    Ok(claims.get_scopes())
}

fn has_server_permission(
    config: &systemprompt_models::services::ServicesConfig,
    server_name: &str,
    user_permissions: &[String],
) -> bool {
    let Some(deployment) = config.mcp_servers.get(server_name) else {
        return true;
    };

    if !deployment.oauth.required || deployment.oauth.scopes.is_empty() {
        return true;
    }

    deployment
        .oauth
        .scopes
        .iter()
        .any(|required_scope| user_permissions.contains(&required_scope.to_string()))
}

async fn load_with_timeout(
    loader: &McpToolLoader,
    server_name: &str,
    context: &RequestContext,
) -> Result<Vec<McpTool>, String> {
    let timeout_duration = tokio::time::Duration::from_secs(10);

    match tokio::time::timeout(
        timeout_duration,
        loader.load_server_tools(server_name, context),
    )
    .await
    {
        Ok(Ok(tools)) => Ok(tools),
        Ok(Err(e)) => {
            let msg = format!("Failed to load tools from MCP server '{server_name}': {e}");
            error!(msg);
            Err(msg)
        },
        Err(_) => {
            let msg =
                format!("Timeout loading tools from MCP server '{server_name}' (exceeded 10s)");
            error!(msg);
            Err(msg)
        },
    }
}

fn log_loading_summary(skipped: &[String], errors: &[String], succeeded: usize, total: usize) {
    if !skipped.is_empty() {
        debug!(
            skipped = %skipped.len(),
            servers = %skipped.join(", "),
            "Skipped servers due to permission restrictions"
        );
    }

    if !errors.is_empty() {
        error!(
            succeeded = %succeeded,
            total = %total,
            failures = %errors.join("; "),
            "Tool loading"
        );
    }
}
