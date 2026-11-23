//! The tool call `plugins mcp call` makes over a [`McpTarget`], and the
//! conversion of a tool result's content blocks into rendered rows.
//!
//! The connection is the probe's: through the API proxy with the caller's
//! own session ([`super::probe`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use anyhow::{Result, anyhow};
use rmcp::model::{CallToolRequestParams, ContentBlock};
use systemprompt_identifiers::McpToolName;
use systemprompt_models::ai::tools::CallToolResult;
use tokio::time::timeout;

use super::probe::{McpTarget, connect};
use super::types::McpToolContent;

#[derive(Debug)]
pub struct ToolCallParams<'a> {
    pub target: &'a McpTarget<'a>,
    pub tool_name: &'a McpToolName,
    // JSON: MCP tool-call arguments are the tool's own JSON object.
    pub arguments: Option<serde_json::Value>,
    pub timeout_secs: u64,
}

pub async fn execute_tool_call(params: ToolCallParams<'_>) -> Result<CallToolResult> {
    let limit = Duration::from_secs(params.timeout_secs);
    let server_name = params.target.server_name();
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
        target,
        tool_name,
        arguments,
        timeout_secs,
    } = params;
    let server_name = target.server_name();

    let client = connect(target, timeout_secs).await?;

    let mut request = CallToolRequestParams::new(String::from(tool_name));
    request.arguments = arguments.and_then(|v| v.as_object().cloned());

    let result = client
        .call_tool(request)
        .await
        .map_err(|e| anyhow!("MCP tool '{tool_name}' on '{server_name}' rejected the call: {e}"))?;

    client.cancel().await?;
    Ok(result)
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
