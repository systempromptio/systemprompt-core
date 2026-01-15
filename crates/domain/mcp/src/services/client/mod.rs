use anyhow::{Context, Result};
use rmcp::handler::client::progress::ProgressDispatcher;
use rmcp::model::{
    ClientCapabilities, ClientInfo, Implementation, ProgressNotificationParam, ProtocolVersion,
};
use rmcp::service::NotificationContext;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use std::time::Duration;
use systemprompt_models::ai::tools::McpTool;
use systemprompt_models::Config;
use tokio::time::timeout;

mod http_client_with_context;
mod types;
mod validation;

use http_client_with_context::HttpClientWithContext;

pub use types::{McpConnectionResult, McpProtocolInfo, ToolExecutionWithId, ValidationResult};
pub use validation::{validate_connection, validate_connection_with_auth};

use crate::models::{ToolExecutionRequest, ToolExecutionResult};
use crate::repository::ToolUsageRepository;
use systemprompt_core_database::DbPool;

#[derive(Debug, Clone)]
pub struct McpClientHandler {
    progress_dispatcher: ProgressDispatcher,
    client_info: ClientInfo,
}

impl McpClientHandler {
    pub fn new(client_info: ClientInfo) -> Self {
        Self {
            progress_dispatcher: ProgressDispatcher::new(),
            client_info,
        }
    }

    pub const fn progress_dispatcher(&self) -> &ProgressDispatcher {
        &self.progress_dispatcher
    }
}

impl ClientHandler for McpClientHandler {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.progress_dispatcher.handle_notification(params).await;
    }

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct McpClient;

impl McpClient {
    pub async fn list_tools(
        service_id: impl Into<String>,
        context: &systemprompt_models::RequestContext,
    ) -> Result<Vec<McpTool>> {
        use crate::services::registry::RegistryManager;

        let service_id = service_id.into();

        RegistryManager::validate()?;
        let server_config = RegistryManager::find_server(&service_id)?
            .ok_or_else(|| anyhow::anyhow!("MCP server '{service_id}' not found in registry"))?;

        let url = server_config.endpoint(&Config::get()?.api_server_url);
        let url = validation::rewrite_url_for_internal_use(&url);
        let requires_auth = server_config.oauth.required;

        let client = HttpClientWithContext::new(context.clone());
        let transport = if requires_auth {
            let user_token = context.auth_token();
            if user_token.as_str().is_empty() {
                return Err(anyhow::anyhow!(
                    "User JWT required for authenticated MCP calls"
                ));
            }
            let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
                .auth_header(format!("Bearer {}", user_token.as_str()));
            StreamableHttpClientTransport::with_client(client, config)
        } else {
            let config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
            StreamableHttpClientTransport::with_client(client, config)
        };

        let client_info = ClientInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "systemprompt-mcp-client".to_string(),
                title: None,
                version: "1.0.0".to_string(),
                website_url: None,
                icons: None,
            },
        };

        let client = client_info.serve(transport).await?;
        let tools_response = client.list_tools(None).await?;

        let tool_metadata = &server_config.tools;

        let mut tools = Vec::new();
        for tool in tools_response.tools {
            let input_schema = serde_json::to_value(tool.input_schema).with_context(|| {
                format!("Failed to serialize input schema for tool '{}'", tool.name)
            })?;

            let output_schema = tool
                .output_schema
                .map(|schema| {
                    serde_json::to_value(schema.as_ref()).with_context(|| {
                        format!("Failed to serialize output schema for tool '{}'", tool.name)
                    })
                })
                .transpose()?;

            let tool_meta = tool_metadata.get(tool.name.as_ref());
            let terminal_on_success = tool_meta.is_some_and(|m| m.terminal_on_success);

            let model_config = tool_meta
                .and_then(|m| m.model_config.clone())
                .or_else(|| server_config.model_config.clone());

            tools.push(McpTool {
                name: tool.name.to_string(),
                description: tool.description.map(|d| d.to_string()),
                input_schema: Some(input_schema),
                output_schema,
                service_id: service_id.clone().into(),
                terminal_on_success,
                model_config,
            });
        }

        client.cancel().await?;
        Ok(tools)
    }

    pub async fn call_tool(
        service_name: &str,
        name: String,
        arguments: Option<serde_json::Value>,
        context: &systemprompt_models::RequestContext,
        db_pool: &DbPool,
    ) -> Result<rmcp::model::CallToolResult> {
        use crate::services::registry::RegistryManager;

        RegistryManager::validate()?;
        let server_config = RegistryManager::find_server(service_name)?
            .ok_or_else(|| anyhow::anyhow!("MCP server '{service_name}' not found in registry"))?;

        let url = server_config.endpoint(&Config::get()?.api_server_url);
        let url = validation::rewrite_url_for_internal_use(&url);

        let tool_repo = ToolUsageRepository::new(db_pool)?;
        let (mcp_execution_id, started_at) =
            start_execution_tracking(&tool_repo, &name, service_name, arguments.clone(), context)
                .await?;

        let transport = build_transport(&url, server_config.oauth.required, context)?;
        let tool_result = execute_tool_call(transport, &name, arguments).await;
        record_execution_result(&tool_repo, &mcp_execution_id, &tool_result, started_at).await?;

        tool_result.map_err(|e| anyhow::anyhow!("Tool execution failed: {e}"))
    }
}

async fn start_execution_tracking(
    tool_repo: &ToolUsageRepository,
    tool_name: &str,
    service_name: &str,
    arguments: Option<serde_json::Value>,
    context: &systemprompt_models::RequestContext,
) -> Result<(
    systemprompt_identifiers::McpExecutionId,
    chrono::DateTime<chrono::Utc>,
)> {
    let started_at = chrono::Utc::now();
    let request = ToolExecutionRequest {
        tool_name: tool_name.to_string(),
        server_name: service_name.to_string(),
        input: arguments.unwrap_or(serde_json::json!({})),
        started_at,
        context: context.clone(),
        request_method: Some("mcp".to_string()),
        request_source: Some("ai_service".to_string()),
        ai_tool_call_id: context.ai_tool_call_id().cloned(),
    };
    let id = tool_repo.start_execution(&request).await?;
    Ok((id, started_at))
}

fn build_transport(
    url: &str,
    requires_auth: bool,
    context: &systemprompt_models::RequestContext,
) -> Result<StreamableHttpClientTransport<HttpClientWithContext>> {
    let client = HttpClientWithContext::new(context.clone());

    if requires_auth {
        let user_token = context.auth_token();
        if user_token.as_str().is_empty() {
            return Err(anyhow::anyhow!(
                "User JWT required for authenticated MCP calls"
            ));
        }
        let config = StreamableHttpClientTransportConfig::with_uri(url)
            .auth_header(format!("Bearer {}", user_token.as_str()));
        Ok(StreamableHttpClientTransport::with_client(client, config))
    } else {
        let config = StreamableHttpClientTransportConfig::with_uri(url);
        Ok(StreamableHttpClientTransport::with_client(client, config))
    }
}

async fn execute_tool_call(
    transport: StreamableHttpClientTransport<HttpClientWithContext>,
    name: &str,
    arguments: Option<serde_json::Value>,
) -> Result<systemprompt_models::CallToolResult, anyhow::Error> {
    let client_info = ClientInfo {
        protocol_version: ProtocolVersion::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "systemprompt-ai-mcp-client".to_string(),
            title: None,
            version: "1.0.0".to_string(),
            website_url: None,
            icons: None,
        },
    };

    let handler = McpClientHandler::new(client_info);

    let client_service = match timeout(Duration::from_secs(30), handler.serve(transport)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            return Err(anyhow::anyhow!(
                "MCP transport serve timed out after 30 seconds"
            ))
        },
    };

    let params = rmcp::model::CallToolRequestParam {
        name: name.to_string().into(),
        arguments: arguments.and_then(|v| v.as_object().cloned()),
    };

    let result = client_service
        .call_tool(params)
        .await
        .map_err(|e| anyhow::anyhow!("MCP tool call failed: {e}"));

    client_service.cancel().await?;
    result
}

async fn record_execution_result(
    tool_repo: &ToolUsageRepository,
    execution_id: &systemprompt_identifiers::McpExecutionId,
    tool_result: &Result<systemprompt_models::CallToolResult, anyhow::Error>,
    started_at: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    let completed_at = chrono::Utc::now();
    let result = match tool_result {
        Ok(res) => ToolExecutionResult {
            output: Some(serde_json::to_value(&res.content).unwrap_or(serde_json::json!({}))),
            output_schema: None,
            status: "success".to_string(),
            error_message: None,
            started_at,
            completed_at,
        },
        Err(e) => ToolExecutionResult {
            output: None,
            output_schema: None,
            status: "failed".to_string(),
            error_message: Some(e.to_string()),
            started_at,
            completed_at,
        },
    };

    tool_repo.complete_execution(execution_id, &result).await
}
