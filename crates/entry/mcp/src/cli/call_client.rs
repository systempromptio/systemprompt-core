//! MCP client for `plugins mcp tools` and `plugins mcp call`.
//!
//! Connects to a running server at its direct address (an internal server's
//! local port, an external server's endpoint), presenting the caller's own
//! session token when one is given, and lists or calls its tools within a
//! deadline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientConfig, ContentBlock, Implementation,
    PaginatedRequestParams, ProtocolVersion,
};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, JwtToken, McpServerId, McpToolName, SessionId, SessionToken,
    TraceId, UserId,
};
use systemprompt_manifest::services::{McpConfig, ResilienceSettings};
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::SpawnTarget;
use systemprompt_mcp::services::client::{HttpClientWithContext, McpClientLimits};
use systemprompt_models::ai::tools::CallToolResult;
use systemprompt_models::execution::context::RequestContext;
use tokio::time::timeout;

use super::types::{McpToolContent, ToolInfo};

#[derive(Debug)]
pub struct ToolCallParams<'a> {
    pub server_name: &'a McpServerId,
    pub url: &'a str,
    pub tool_name: &'a McpToolName,
    // JSON: MCP tool-call arguments are the tool's own JSON object.
    pub arguments: Option<serde_json::Value>,
    pub token: &'a SessionToken,
    pub timeout_secs: u64,
}

pub fn direct_url(server: &McpServerConfig) -> Result<String> {
    if server.is_external() {
        return Ok(server.remote_endpoint.clone());
    }
    Ok(format!("http://127.0.0.1:{}/mcp", server.spawn_port()?))
}

pub fn cli_limits(timeout_secs: u64) -> McpClientLimits {
    McpClientLimits::from(&ResilienceSettings {
        request_timeout_ms: timeout_secs.saturating_mul(1000),
        ..McpConfig::default().resilience
    })
}

pub async fn execute_tool_call(params: ToolCallParams<'_>) -> Result<CallToolResult> {
    let limit = Duration::from_secs(params.timeout_secs);
    let server_name = params.server_name;
    let tool_name = params.tool_name;
    timeout(limit, call_tool(params)).await.map_err(|elapsed| {
        anyhow!(
            "MCP tool '{tool_name}' on '{server_name}' did not finish within {}s ({elapsed})",
            limit.as_secs()
        )
    })?
}

async fn call_tool(params: ToolCallParams<'_>) -> Result<CallToolResult> {
    let ToolCallParams {
        server_name,
        url,
        tool_name,
        arguments,
        token,
        timeout_secs,
    } = params;

    let client = connect(server_name, url, Some(token), timeout_secs).await?;

    let mut request = CallToolRequestParams::new(String::from(tool_name));
    request.arguments = arguments.and_then(|v| v.as_object().cloned());

    let result = client
        .call_tool(request)
        .await
        .map_err(|e| anyhow!("MCP tool '{tool_name}' on '{server_name}' rejected the call: {e}"))?;

    client.cancel().await?;
    Ok(result)
}

pub async fn list_tools(
    server_name: &McpServerId,
    url: &str,
    token: Option<&SessionToken>,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let limit = Duration::from_secs(timeout_secs);
    timeout(limit, fetch_tools(server_name, url, token, timeout_secs))
        .await
        .map_err(|elapsed| {
            anyhow!(
                "Listing tools on '{server_name}' did not finish within {}s ({elapsed})",
                limit.as_secs()
            )
        })?
}

async fn fetch_tools(
    server_name: &McpServerId,
    url: &str,
    token: Option<&SessionToken>,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let client = connect(server_name, url, token, timeout_secs).await?;

    let tools_response = client
        // Why: rmcp serialises a `None` params as `"params": null`, which a
        // strict server (Google's MCP) refuses with -32602; send `{}`.
        .list_tools(Some(PaginatedRequestParams::default()))
        .await
        .context("Failed to list tools")?;

    let tools = tools_response
        .tools
        .into_iter()
        .map(ToolInfo::from_tool)
        .collect();

    client.cancel().await?;
    Ok(tools)
}

fn request_context(server_name: &McpServerId, token: Option<&SessionToken>) -> RequestContext {
    let context = RequestContext::new(
        SessionId::new(format!("cli-{server_name}")),
        TraceId::generate(),
        ContextId::derived_from_cli_probe(server_name),
        AgentName::system(),
        Actor::anonymous(UserId::generate()),
    );
    match token {
        Some(token) => context.with_auth_token(JwtToken::new(token.as_str())),
        None => context,
    }
}

async fn connect(
    server_name: &McpServerId,
    url: &str,
    token: Option<&SessionToken>,
    timeout_secs: u64,
) -> Result<RunningService<RoleClient, ClientConfig>> {
    let http_client = HttpClientWithContext::new(
        request_context(server_name, token),
        cli_limits(timeout_secs),
    )?;
    let config = match token {
        Some(token) => StreamableHttpClientTransportConfig::with_uri(url)
            .auth_header(token.as_str().to_owned()),
        None => StreamableHttpClientTransportConfig::with_uri(url),
    };
    let transport = StreamableHttpClientTransport::with_client(http_client, config);

    let client_info = ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new(format!("systemprompt-cli-{server_name}"), "1.0.0"),
    )
    .with_protocol_version(ProtocolVersion::LATEST_WITH_INITIALIZE);

    client_info
        .serve(transport)
        .await
        .context("Failed to connect to MCP server")
}

pub fn convert_content(content: &ContentBlock) -> Result<McpToolContent, serde_json::Error> {
    Ok(match content {
        ContentBlock::Text(text) => McpToolContent {
            kind: "text".to_owned(),
            text: Some(text.text.clone()),
            mime_type: None,
            data: None,
        },
        ContentBlock::Image(image) => McpToolContent {
            kind: "image".to_owned(),
            text: None,
            mime_type: Some(image.mime_type.clone()),
            data: Some(image.data.clone()),
        },
        ContentBlock::Resource(resource) => McpToolContent {
            kind: "resource".to_owned(),
            text: Some(serde_json::to_string(&resource.resource)?),
            mime_type: None,
            data: None,
        },
        ContentBlock::Audio(audio) => McpToolContent {
            kind: "audio".to_owned(),
            text: None,
            mime_type: Some(audio.mime_type.clone()),
            data: Some(audio.data.clone()),
        },
        ContentBlock::ResourceLink(link) => McpToolContent {
            kind: "resource_link".to_owned(),
            text: Some(link.uri.clone()),
            mime_type: link.mime_type.clone(),
            data: None,
        },
        _ => McpToolContent {
            kind: "unknown".to_owned(),
            text: None,
            mime_type: None,
            data: None,
        },
    })
}
