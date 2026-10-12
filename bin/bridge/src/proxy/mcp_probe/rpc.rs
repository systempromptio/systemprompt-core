//! MCP JSON-RPC exchanges used by the probe: server/discover, the legacy
//! initialize handshake, tools/list, parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;

use super::{LEGACY_PROTOCOL_VERSION, MCP_PROTOCOL_VERSION, McpTool, SESSION_HEADER};

const MCP_METHOD_HEADER: &str = "mcp-method";
const PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";

pub(super) enum Lifecycle<'a> {
    Stateless,
    Session(Option<&'a str>),
}

pub(super) fn post(client: &reqwest::Client, url: &str, bearer: &str) -> reqwest::RequestBuilder {
    client
        .post(url)
        .header(AUTHORIZATION, bearer)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
}

pub(super) fn stateless(builder: reqwest::RequestBuilder, method: &str) -> reqwest::RequestBuilder {
    builder
        .header(MCP_METHOD_HEADER, method)
        .header(PROTOCOL_VERSION_HEADER, MCP_PROTOCOL_VERSION)
}

pub(super) async fn list_tools(
    client: &reqwest::Client,
    url: &str,
    bearer: &str,
    lifecycle: Lifecycle<'_>,
) -> Result<Vec<McpTool>, RpcError> {
    let req = match lifecycle {
        Lifecycle::Stateless => {
            stateless(post(client, url, bearer), "tools/list").json(&JsonRpcRequest {
                jsonrpc: "2.0",
                id: 2,
                method: "tools/list",
                params: MetaParams::current(),
            })
        },
        Lifecycle::Session(session) => {
            with_session(post(client, url, bearer), session)
                .json(&JsonRpcNotification {
                    jsonrpc: "2.0",
                    method: "notifications/initialized",
                })
                .send()
                .await?
                .error_for_status()?;
            with_session(post(client, url, bearer), session).json(&JsonRpcRequest {
                jsonrpc: "2.0",
                id: 2,
                method: "tools/list",
                // JSON: `params` must be an object; `null` is answered with
                // `-32602 Invalid params` by Google's MCP endpoint.
                params: serde_json::Map::<String, Value>::new(),
            })
        },
    };
    let resp = req.send().await?.error_for_status()?;
    let content_type = content_type(&resp);
    let body = resp.text().await?;
    parse_tools(&content_type, &body)
}

pub(super) fn content_type(resp: &reqwest::Response) -> String {
    resp.headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
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

impl ClientInfo {
    fn probe() -> Self {
        Self {
            name: "systemprompt-bridge-probe",
            version: crate::brand::brand().version,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MetaParams {
    #[serde(rename = "_meta")]
    pub meta: RequestMeta,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct RequestMeta {
    #[serde(rename = "io.modelcontextprotocol/protocolVersion")]
    pub protocol_version: &'static str,
    // JSON: MCP client capabilities object; the probe advertises none.
    #[serde(rename = "io.modelcontextprotocol/clientCapabilities")]
    pub client_capabilities: serde_json::Map<String, Value>,
    #[serde(rename = "io.modelcontextprotocol/clientInfo")]
    pub client_info: ClientInfo,
}

impl MetaParams {
    fn current() -> Self {
        Self {
            meta: RequestMeta {
                protocol_version: MCP_PROTOCOL_VERSION,
                client_capabilities: serde_json::Map::new(),
                client_info: ClientInfo::probe(),
            },
        }
    }
}

pub(super) fn discover_body() -> JsonRpcRequest<MetaParams> {
    JsonRpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "server/discover",
        params: MetaParams::current(),
    }
}

#[derive(Debug, serde::Deserialize)]
struct DiscoverResponse {
    result: Option<DiscoverResult>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverResult {
    #[serde(default)]
    supported_versions: Vec<String>,
}

pub(super) fn discover_supports_current(content_type: &str, body: &str) -> bool {
    serde_json::from_str::<DiscoverResponse>(&event_data(content_type, body))
        .ok()
        .and_then(|response| response.result)
        .is_some_and(|result| {
            result
                .supported_versions
                .iter()
                .any(|version| version == MCP_PROTOCOL_VERSION)
        })
}


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
            protocol_version: LEGACY_PROTOCOL_VERSION,
            capabilities: serde_json::Map::new(),
            client_info: ClientInfo::probe(),
        },
    }
}

#[derive(Debug, serde::Deserialize)]
struct ToolsListResponse {
    result: Option<ToolsListResult>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, serde::Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
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
    #[error("MCP tools/list returned JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("MCP tools/list response did not contain a result.tools array")]
    MissingTools,
}

fn event_data(content_type: &str, body: &str) -> String {
    if content_type.contains("text/event-stream") {
        body.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        body.to_owned()
    }
}

fn parse_tools(content_type: &str, body: &str) -> Result<Vec<McpTool>, RpcError> {
    let response: ToolsListResponse = serde_json::from_str(&event_data(content_type, body))?;
    if let Some(error) = response.error {
        return Err(RpcError::Rpc {
            code: error.code,
            message: error.message,
        });
    }
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
