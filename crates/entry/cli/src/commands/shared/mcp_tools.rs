//! MCP tool probing shared by `admin agents tools` and `plugins mcp tools`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::time::Duration;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, SessionToken, TraceId};
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::SpawnTarget;
use systemprompt_mcp::services::client::HttpClientWithContext;
use systemprompt_models::execution::context::RequestContext;
use tokio::time::timeout;
use tracing::debug;

fn probe_context(server_name: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("cli-{server_name}")),
        TraceId::generate(),
        ContextId::derived_from_cli_probe(server_name),
        AgentName::system(),
    )
}

#[derive(Debug)]
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub parameters_count: usize,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}

// Why: an internal server is spawned on a local port and reached there
// directly; an external one is never spawned and is reached at its endpoint.
pub fn direct_url(server: &McpServerConfig) -> Result<String> {
    if server.is_external() {
        return Ok(server.remote_endpoint.clone());
    }
    Ok(format!("http://127.0.0.1:{}/mcp", server.spawn_port()?))
}

pub async fn list_tools_unauthenticated(
    server_name: &str,
    url: &str,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let transport = StreamableHttpClientTransport::with_client(
        HttpClientWithContext::new(probe_context(server_name))?,
        config,
    );

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new(format!("systemprompt-cli-{}", server_name), "1.0.0"),
    );

    let client = timeout(
        Duration::from_secs(timeout_secs),
        client_info.serve(transport),
    )
    .await
    .context("Connection timeout")?
    .context("Failed to connect to MCP server")?;

    let tools_response = client
        // Why: rmcp serialises a `None` params as `"params": null`, which a
        // strict server (Google's MCP) refuses with -32602; send `{}`.
        .list_tools(Some(rmcp::model::PaginatedRequestParams::default()))
        .await
        .context("Failed to list tools")?;

    let tools: Vec<ToolInfo> = tools_response
        .tools
        .into_iter()
        .map(convert_tool_info)
        .collect();

    client.cancel().await?;
    Ok(tools)
}

pub async fn list_tools_authenticated(
    server_name: &str,
    url: &str,
    token: &SessionToken,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let config =
        StreamableHttpClientTransportConfig::with_uri(url).auth_header(token.as_str().to_owned());
    let context = probe_context(server_name).with_auth_token(token.as_str());
    let transport =
        StreamableHttpClientTransport::with_client(HttpClientWithContext::new(context)?, config);

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new(format!("systemprompt-cli-{}", server_name), "1.0.0"),
    );

    let client = timeout(
        Duration::from_secs(timeout_secs),
        client_info.serve(transport),
    )
    .await
    .context("Connection timeout")?
    .context("Failed to connect to MCP server")?;

    let tools_response = client
        // Why: rmcp serialises a `None` params as `"params": null`, which a
        // strict server (Google's MCP) refuses with -32602; send `{}`.
        .list_tools(Some(rmcp::model::PaginatedRequestParams::default()))
        .await
        .context("Failed to list tools")?;

    let tools: Vec<ToolInfo> = tools_response
        .tools
        .into_iter()
        .map(convert_tool_info)
        .collect();

    client.cancel().await?;
    Ok(tools)
}

fn convert_tool_info(tool: rmcp::model::Tool) -> ToolInfo {
    let input_schema = match serde_json::to_value(&tool.input_schema) {
        Ok(schema) => Some(schema),
        Err(e) => {
            debug!(tool = %tool.name, error = %e, "Failed to serialize input schema");
            None
        },
    };
    let output_schema =
        tool.output_schema
            .as_ref()
            .and_then(|s| match serde_json::to_value(s.as_ref()) {
                Ok(schema) => Some(schema),
                Err(e) => {
                    debug!(tool = %tool.name, error = %e, "Failed to serialize output schema");
                    None
                },
            });
    let parameters_count = input_schema
        .as_ref()
        .and_then(|s| s.get("properties"))
        .and_then(|p| p.as_object())
        .map_or(0, serde_json::Map::len);

    ToolInfo {
        name: tool.name.to_string(),
        description: tool.description.map(|d| d.to_string()),
        parameters_count,
        input_schema,
        output_schema,
    }
}
