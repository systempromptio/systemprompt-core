//! MCP JSON-RPC exchanges used by the probe: initialize, tools/list, parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value, json};

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
        .json(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
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
        .json(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }))
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

pub(super) fn initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "systemprompt-bridge-probe", "version": crate::brand::brand().version },
        },
    })
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
    #[derive(serde::Deserialize)]
    struct Tool {
        name: String,
        description: Option<String>,
    }
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
    let value: Value = serde_json::from_str(body)?;
    let tools = value
        .get("result")
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .ok_or(RpcError::MissingTools)?;
    tools
        .iter()
        .map(|value| {
            let tool: Tool = serde_json::from_value(value.clone())?;
            Ok(McpTool {
                name: tool.name,
                description: tool.description,
            })
        })
        .collect()
}
