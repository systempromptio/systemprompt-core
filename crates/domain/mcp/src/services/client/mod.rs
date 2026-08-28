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
use rmcp::ServiceExt;
use rmcp::model::{ClientInfo, Implementation, ProtocolVersion};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use systemprompt_identifiers::McpServerId;
use systemprompt_models::Config;
use systemprompt_models::ai::tools::McpTool;

mod bounded_sse;
mod capabilities;
mod challenge;
mod elicitation;
pub mod external_auth;
mod external_proxy;
mod handler;
mod http_client_with_context;
mod invocation;
mod tasks;
mod types;
mod validation;

pub use challenge::{AuthChallenge, McpTransportError};
pub use elicitation::{ElicitationDelegate, SharedElicitationDelegate};
pub use external_proxy::ExternalProxyTarget;
pub use handler::McpClientHandler;
pub use http_client_with_context::HttpClientWithContext;
pub use invocation::execute_tool_call;
pub use types::{McpConnectionResult, McpProtocolInfo, ValidationResult};
pub use validation::{
    rewrite_url_for_internal_use, validate_connection, validate_connection_by_url,
    validate_connection_with_auth,
};

#[derive(Debug, Clone, Copy)]
pub struct McpClient;

impl McpClient {
    pub async fn list_tools(
        server_config: &systemprompt_models::mcp::McpServerConfig,
        context: &systemprompt_models::RequestContext,
    ) -> McpDomainResult<Vec<McpTool>> {
        let service_id = server_config.name.as_str();
        let transport = build_transport(server_config, context, false).await?;

        let client_info = ClientInfo::new(
            capabilities::client_capabilities(false),
            Implementation::new("systemprompt-mcp-client", "1.0.0"),
        )
        .with_protocol_version(ProtocolVersion::V_2026_07_28);

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
        let transport = build_transport(server_config, context, elicitation.is_some()).await?;
        execute_tool_call(transport, service_name, &name, arguments, elicitation).await
    }
}

async fn build_transport(
    server_config: &systemprompt_models::mcp::McpServerConfig,
    context: &systemprompt_models::RequestContext,
    with_elicitation: bool,
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
            .with_client_capabilities(capabilities::client_capabilities(with_elicitation))
    } else {
        if server_config.oauth.required {
            let user_token = context.auth_token();
            if user_token.as_str().is_empty() {
                return Err(crate::error::McpDomainError::AuthRequired(
                    "User JWT required for authenticated MCP calls".to_owned(),
                ));
            }
            // Why: `auth_header` takes the bare token — the transport calls
            // `bearer_auth` on it, so a pre-formatted header reaches the
            // server as "Bearer Bearer <jwt>" and is rejected as malformed.
            transport_config = transport_config.auth_header(user_token.as_str().to_owned());
        }
        let outbound =
            external_auth::static_outbound_headers(&server_config.headers, &server_config.name)?;
        HttpClientWithContext::forwarding(context.clone(), outbound)
            .with_client_capabilities(capabilities::client_capabilities(with_elicitation))
    };

    Ok(StreamableHttpClientTransport::with_client(
        client,
        transport_config,
    ))
}
