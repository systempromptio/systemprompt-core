//! MCP client.
//!
//! Connects to running MCP servers over streamable HTTP, lists their tools,
//! executes tool calls, and validates reachability.
//!
//! Sampling and roots are neither implemented nor advertised in
//! [`rmcp::model::ClientCapabilities`] (both deprecated by SEP-2577, and
//! servicing `create_message` would let a third-party MCP server spend our
//! inference budget under our credentials). Elicitation is advertised only
//! when an [`ElicitationDelegate`] is installed; the
//! `io.modelcontextprotocol/tasks` extension is always advertised and task
//! handles are polled to completion.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use rmcp::handler::client::progress::ProgressDispatcher;
use rmcp::model::{ClientInfo, Implementation, ProgressNotificationParam};
use rmcp::service::NotificationContext;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use systemprompt_identifiers::McpServerId;
use systemprompt_models::Config;
use systemprompt_models::ai::tools::McpTool;
use systemprompt_models::net::{HTTP_STREAM_CONNECT_TIMEOUT, MCP_TOOL_EXECUTION_TIMEOUT};
use tokio::time::timeout;

mod bounded_sse;
mod capabilities;
mod challenge;
mod elicitation;
pub mod external_auth;
mod external_proxy;
mod http_client_with_context;
mod tasks;
mod types;
mod validation;

pub use challenge::{AuthChallenge, McpTransportError};
pub use elicitation::{ElicitationDelegate, SharedElicitationDelegate};
pub use external_proxy::ExternalProxyTarget;
pub use http_client_with_context::HttpClientWithContext;
pub use types::{McpConnectionResult, McpProtocolInfo, ValidationResult};
pub use validation::{
    rewrite_url_for_internal_use, validate_connection, validate_connection_by_url,
    validate_connection_with_auth,
};

#[derive(Debug, Clone)]
pub struct McpClientHandler {
    progress_dispatcher: ProgressDispatcher,
    client_info: ClientInfo,
    elicitation: Option<SharedElicitationDelegate>,
}

impl McpClientHandler {
    pub fn new(client_info: ClientInfo) -> Self {
        Self {
            progress_dispatcher: ProgressDispatcher::new(),
            client_info,
            elicitation: None,
        }
    }

    #[must_use]
    pub fn with_elicitation(mut self, delegate: SharedElicitationDelegate) -> Self {
        self.elicitation = Some(delegate);
        self
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

    async fn create_elicitation(
        &self,
        params: rmcp::model::ElicitRequestParams,
        _context: rmcp::service::RequestContext<RoleClient>,
    ) -> Result<rmcp::model::ElicitResult, rmcp::ErrorData> {
        Ok(elicitation::handle_elicitation(self.elicitation.as_ref(), params).await)
    }

    async fn on_task_status(
        &self,
        params: rmcp::model::TaskStatusNotificationParams,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            task_id = %params.task.task.task_id,
            status = ?params.task.task.status,
            "Task status notification received"
        );
    }

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct McpClient;

impl McpClient {
    pub async fn list_tools(
        server_config: &systemprompt_models::mcp::McpServerConfig,
        context: &systemprompt_models::RequestContext,
    ) -> McpDomainResult<Vec<McpTool>> {
        let service_id = server_config.name.as_str();
        let transport = build_transport(server_config, context).await?;

        let client_info = ClientInfo::new(
            capabilities::client_capabilities(false),
            Implementation::new("systemprompt-mcp-client", "1.0.0"),
        );

        let client = client_info.serve(transport).await?;
        let all_tools = client.list_all_tools().await?;

        let tool_metadata = &server_config.tools;

        let mut tools = Vec::new();
        for tool in all_tools {
            let input_schema = serde_json::to_value(tool.input_schema).map_err(|e| {
                crate::error::McpDomainError::Internal(format!("{}: {e}", {
                    format!("Failed to serialize input schema for tool '{}'", tool.name)
                }))
            })?;

            let output_schema = tool
                .output_schema
                .map(|schema| {
                    serde_json::to_value(schema.as_ref()).map_err(|e| {
                        crate::error::McpDomainError::Internal(format!("{}: {e}", {
                            format!("Failed to serialize output schema for tool '{}'", tool.name)
                        }))
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
                service_id: McpServerId::new(service_id),
                terminal_on_success,
                model_config,
            });
        }

        client.cancel().await?;
        Ok(tools)
    }

    pub async fn call_tool(
        server_config: &systemprompt_models::mcp::McpServerConfig,
        name: String,
        arguments: Option<serde_json::Value>,
        context: &systemprompt_models::RequestContext,
    ) -> McpDomainResult<systemprompt_models::CallToolResult> {
        Self::call_tool_with_elicitation(server_config, name, arguments, context, None).await
    }

    pub async fn call_tool_with_elicitation(
        server_config: &systemprompt_models::mcp::McpServerConfig,
        name: String,
        arguments: Option<serde_json::Value>,
        context: &systemprompt_models::RequestContext,
        elicitation: Option<SharedElicitationDelegate>,
    ) -> McpDomainResult<systemprompt_models::CallToolResult> {
        let service_name = server_config.name.as_str();
        let transport = build_transport(server_config, context).await?;
        execute_tool_call(transport, service_name, &name, arguments, elicitation).await
    }
}

async fn build_transport(
    server_config: &systemprompt_models::mcp::McpServerConfig,
    context: &systemprompt_models::RequestContext,
) -> McpDomainResult<StreamableHttpClientTransport<HttpClientWithContext>> {
    let raw_url = server_config.call_url(&Config::get()?.api_server_url);
    let url = if server_config.is_external() {
        raw_url
    } else {
        rewrite_url_for_internal_use(&raw_url)
    };

    let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url.as_str());

    let client = if let Some(ext) = server_config.external_auth.as_ref() {
        let bearer =
            external_auth::resolve_external_bearer(ext, context, &server_config.name).await?;
        let outbound = external_auth::outbound_headers(
            ext,
            &bearer,
            &server_config.headers,
            &server_config.name,
        )?;
        HttpClientWithContext::external(context.clone(), outbound)
    } else {
        if server_config.oauth.required {
            let user_token = context.auth_token();
            if user_token.as_str().is_empty() {
                return Err(crate::error::McpDomainError::AuthRequired(
                    "User JWT required for authenticated MCP calls".to_owned(),
                ));
            }
            transport_config =
                transport_config.auth_header(format!("Bearer {}", user_token.as_str()));
        }
        let outbound =
            external_auth::static_outbound_headers(&server_config.headers, &server_config.name)?;
        HttpClientWithContext::forwarding(context.clone(), outbound)
    };

    Ok(StreamableHttpClientTransport::with_client(
        client,
        transport_config,
    ))
}

pub async fn execute_tool_call(
    transport: StreamableHttpClientTransport<HttpClientWithContext>,
    server: &str,
    name: &str,
    arguments: Option<serde_json::Value>,
    elicitation: Option<SharedElicitationDelegate>,
) -> McpDomainResult<systemprompt_models::CallToolResult> {
    let client_info = ClientInfo::new(
        capabilities::client_capabilities(elicitation.is_some()),
        Implementation::new("systemprompt-ai-mcp-client", "1.0.0"),
    );

    let mut handler = McpClientHandler::new(client_info);
    if let Some(delegate) = elicitation {
        handler = handler.with_elicitation(delegate);
    }

    let client_service = match timeout(HTTP_STREAM_CONNECT_TIMEOUT, handler.serve(transport)).await
    {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            return Err(crate::error::McpDomainError::Timeout {
                server: server.to_owned(),
                after_ms: u64::try_from(HTTP_STREAM_CONNECT_TIMEOUT.as_millis())
                    .unwrap_or(u64::MAX),
            });
        },
    };

    let mut params = rmcp::model::CallToolRequestParams::new(name.to_owned());
    if let Some(args) = arguments.and_then(|v| v.as_object().cloned()) {
        params = params.with_arguments(args);
    }

    let result = timeout(
        MCP_TOOL_EXECUTION_TIMEOUT,
        dispatch_tool_call(&client_service, server, params),
    )
    .await
    .unwrap_or_else(|_| {
        Err(crate::error::McpDomainError::Timeout {
            server: server.to_owned(),
            after_ms: u64::try_from(MCP_TOOL_EXECUTION_TIMEOUT.as_millis()).unwrap_or(u64::MAX),
        })
    });

    client_service.cancel().await?;
    result
}

async fn dispatch_tool_call<S>(
    client: &rmcp::service::RunningService<RoleClient, S>,
    server: &str,
    params: rmcp::model::CallToolRequestParams,
) -> McpDomainResult<systemprompt_models::CallToolResult>
where
    S: rmcp::Service<RoleClient>,
{
    use rmcp::model::CallToolResponse;

    match client.call_tool_once(params.clone()).await.map_err(|e| {
        crate::error::McpDomainError::ToolExecutionFailed(format!("MCP tool call failed: {e}"))
    })? {
        CallToolResponse::Complete(result) => Ok(result),
        CallToolResponse::Task(created) => {
            tasks::poll_task_to_completion(client, server, created).await
        },
        // Why: rmcp's MRTR driver owns the input_required loop, but its retry
        // assembly is private, so the round is re-entered through `call_tool`;
        // SEP-2322 rounds are stateless on the server, so the extra initial
        // round-trip is harmless.
        CallToolResponse::InputRequired(_) => client.call_tool(params).await.map_err(|e| {
            crate::error::McpDomainError::ToolExecutionFailed(format!("MCP tool call failed: {e}"))
        }),
        other => Err(crate::error::McpDomainError::ToolExecutionFailed(format!(
            "unexpected tools/call response variant: {other:?}"
        ))),
    }
}
