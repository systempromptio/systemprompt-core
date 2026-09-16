//! MCP JSON-RPC exchanges used by the probe: initialize, tools/list, parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;

use super::{MCP_PROTOCOL_VERSION, McpTool, SESSION_HEADER};

pub(super) async fn list_tools(
    client: &reqwest::Client,
    url: &str,
    bearer: &str,
    session: Option<&str>,
) -> Result<Vec<McpTool>, RpcError> {
    let initialized = with_session(
        client
            .post(url)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream"),
        session,
    );
    initialized
        .json(&JsonRpcNotification {
            jsonrpc: "2.0",
            method: "notifications/initialized",
        })
        .send()
        .await?
        .error_for_status()?;

    let req = with_session(
        client
            .post(url)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream"),
        session,
    );
    let resp = req
        .json(&JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "tools/list",
            params: EmptyParams {},
        })
        .send()
        .await?
        .error_for_status()?;
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = resp.text().await?;
    parse_tools(&content_type, &body)
}

fn with_session(
    builder: reqwest::RequestBuilder,
    session: Option<&str>,
) -> reqwest::RequestBuilder {
    match session {
        Some(s) => builder.header(SESSION_HEADER, s),
        None => builder,
    }
}

#[derive(Debug, serde::Serialize)]
pub(super) struct JsonRpcRequest<P> {
    pub jsonrpc: &'static str,
    pub id: u32,
    pub method: &'static str,
    pub params: P,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InitializeParams {
    pub protocol_version: &'static str,
    // JSON: MCP `initialize` capabilities object; the probe advertises none.
    pub capabilities: serde_json::Map<String, Value>,
    pub client_info: ClientInfo,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct ClientInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct EmptyParams;

#[derive(Debug, serde::Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: &'static str,
}

pub(super) fn initialize_body() -> JsonRpcRequest<InitializeParams> {
    JsonRpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION,
            capabilities: serde_json::Map::new(),
            client_info: ClientInfo {
                name: "systemprompt-bridge-probe",
                version: crate::brand::brand().version,
            },
        },
    }
}

#[derive(Debug, serde::Deserialize)]
struct ToolsListResponse {
    result: Option<ToolsListResult>,
}

#[derive(Debug, serde::Deserialize)]
struct ToolsListResult {
    tools: Option<Vec<Tool>>,
}

#[derive(Debug, serde::Deserialize)]
struct Tool {
    name: String,
    description: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum RpcError {
    #[error("MCP tools/list transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("MCP tools/list response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("MCP tools/list response did not contain a result.tools array")]
    MissingTools,
}

fn parse_tools(content_type: &str, body: &str) -> Result<Vec<McpTool>, RpcError> {
    let data;
    let body = if content_type.contains("text/event-stream") {
        data = body
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        data.as_str()
    } else {
        body
    };
    let response: ToolsListResponse = serde_json::from_str(body)?;
    let tools = response
        .result
        .and_then(|result| result.tools)
        .ok_or(RpcError::MissingTools)?;
    Ok(tools
        .into_iter()
        .map(|tool| McpTool {
            name: tool.name,
            description: tool.description,
        })
        .collect())
}
