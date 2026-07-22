//! MCP client for `plugins mcp call`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientInfo, ContentBlock, Implementation,
};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::time::Duration;
use systemprompt_mcp::services::client::HttpClientWithContext;
use systemprompt_models::ai::tools::CallToolResult;
use tokio::time::timeout;

use super::types::McpToolContent;
use crate::session::CliSessionContext;

#[derive(Debug)]
pub struct ToolCallParams<'a> {
    pub server_name: &'a str,
    pub port: u16,
    pub tool_name: &'a str,
    pub arguments: Option<serde_json::Value>,
    pub session_ctx: &'a CliSessionContext,
    pub timeout_secs: u64,
}

pub async fn execute_tool_call(params: ToolCallParams<'_>) -> Result<CallToolResult> {
    let ToolCallParams {
        server_name,
        port,
        tool_name,
        arguments,
        session_ctx,
        timeout_secs,
    } = params;
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let request_context = session_ctx.to_request_context(&format!("cli-{}", server_name));
    let http_client = HttpClientWithContext::new(request_context);
    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
        .auth_header(format!("Bearer {}", session_ctx.session_token().as_str()));
    let transport = StreamableHttpClientTransport::with_client(http_client, config);

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

    let mut params = CallToolRequestParams::new(tool_name.to_owned());
    params.arguments = arguments.and_then(|v| v.as_object().cloned());

    let result = client.call_tool(params).await.map_err(|e| {
        anyhow::anyhow!(
            "MCP tool '{}' on '{}' rejected the call: {}",
            tool_name,
            server_name,
            e
        )
    })?;

    client.cancel().await?;
    Ok(result)
}

pub async fn list_available_tools(
    server_name: &str,
    port: u16,
    session_ctx: &CliSessionContext,
    timeout_secs: u64,
) -> Result<Vec<String>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let request_context = session_ctx.to_request_context(&format!("cli-{}", server_name));
    let http_client = HttpClientWithContext::new(request_context);
    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
        .auth_header(format!("Bearer {}", session_ctx.session_token().as_str()));
    let transport = StreamableHttpClientTransport::with_client(http_client, config);

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
        .list_tools(None)
        .await
        .context("Failed to list tools")?;

    let tool_names: Vec<String> = tools_response
        .tools
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();

    client.cancel().await?;
    Ok(tool_names)
}

pub fn convert_content(content: &ContentBlock) -> McpToolContent {
    match content {
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
            text: Some(format!("{:?}", resource.resource)),
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
    }
}
