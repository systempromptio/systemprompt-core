//! MCP client for `plugins mcp tools` and `plugins mcp call`.
//!
//! Connects to a running server at its direct address (an internal server's
//! local port, an external server's endpoint) and lists or calls its tools
//! within a deadline. What the client presents is decided per server by
//! [`server_credential`]: a server the platform hosts receives the caller's
//! own session (token, session, user and context ids), so the execution is
//! attributed to that caller; an external server never receives the platform
//! session or the caller's ids. An external server that requires a credential
//! is refused with [`CredentialRefused`] naming it: its per-user bearer is
//! injected only by the gateway route `/api/v1/mcp/<server>/mcp`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
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
use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, McpServerId, McpToolName, SessionId, TraceId, UserId,
};
use systemprompt_manifest::services::{McpConfig, ResilienceSettings};
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::SpawnTarget;
use systemprompt_mcp::services::client::{HttpClientWithContext, McpClientLimits};
use systemprompt_models::ai::tools::CallToolResult;
use systemprompt_models::execution::context::RequestContext;
use tokio::time::timeout;

use super::types::{McpToolContent, ToolInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerCredential {
    CallerSession,
    Withheld,
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialRefused {
    #[error(
        "MCP server '{server}' is external and requires a credential; the platform session is \
         presented only to servers the platform hosts. Reach it through the gateway at \
         /api/v1/mcp/{server}/mcp, which injects its own per-user bearer"
    )]
    ExternalServer { server: McpServerId },
    #[error("MCP server '{server}' is hosted by the platform and needs the caller's session")]
    MissingCallerSession { server: McpServerId },
}

pub fn server_credential(server: &McpServerConfig) -> Result<ServerCredential, CredentialRefused> {
    if !server.is_external() {
        return Ok(ServerCredential::CallerSession);
    }
    if server.oauth.required || server.external_auth.is_some() {
        return Err(CredentialRefused::ExternalServer {
            server: server.server_id(),
        });
    }
    Ok(ServerCredential::Withheld)
}

#[derive(Debug)]
pub struct McpTarget<'a> {
    server_name: McpServerId,
    url: String,
    session: Option<&'a CallerSession>,
}

impl<'a> McpTarget<'a> {
    pub fn resolve(server: &McpServerConfig, session: Option<&'a CallerSession>) -> Result<Self> {
        let session = match server_credential(server)? {
            ServerCredential::CallerSession => {
                Some(
                    session.ok_or_else(|| CredentialRefused::MissingCallerSession {
                        server: server.server_id(),
                    })?,
                )
            },
            ServerCredential::Withheld => None,
        };
        Ok(Self {
            server_name: server.server_id(),
            url: direct_url(server)?,
            session,
        })
    }

    #[must_use]
    pub const fn server_name(&self) -> &McpServerId {
        &self.server_name
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub const fn presents_caller_session(&self) -> bool {
        self.session.is_some()
    }
}

#[derive(Debug)]
pub struct ToolCallParams<'a> {
    pub target: &'a McpTarget<'a>,
    pub tool_name: &'a McpToolName,
    // JSON: MCP tool-call arguments are the tool's own JSON object.
    pub arguments: Option<serde_json::Value>,
    pub timeout_secs: u64,
}

fn direct_url(server: &McpServerConfig) -> Result<String> {
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

pub async fn list_tools(target: &McpTarget<'_>, timeout_secs: u64) -> Result<Vec<ToolInfo>> {
    let limit = Duration::from_secs(timeout_secs);
    let server_name = target.server_name();
    timeout(limit, fetch_tools(target, timeout_secs))
        .await
        .map_err(|elapsed| {
            anyhow!(
                "Listing tools on '{server_name}' did not finish within {}s ({elapsed})",
                limit.as_secs()
            )
        })?
}

async fn fetch_tools(target: &McpTarget<'_>, timeout_secs: u64) -> Result<Vec<ToolInfo>> {
    let client = connect(target, timeout_secs).await?;

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

fn withheld_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::system(),
        Actor::anonymous(UserId::generate()),
    )
}

async fn connect(
    target: &McpTarget<'_>,
    timeout_secs: u64,
) -> Result<RunningService<RoleClient, ClientConfig>> {
    let server_name = target.server_name();
    let limits = cli_limits(timeout_secs);
    let config = StreamableHttpClientTransportConfig::with_uri(target.url.clone());
    let (http_client, config) = match target.session {
        Some(session) => {
            let agent_name = AgentName::try_new(format!("cli-{server_name}"))
                .context("MCP server name does not form a valid agent name")?;
            (
                HttpClientWithContext::new(session.request_context(agent_name), limits)?,
                config.auth_header(session.token().as_str().to_owned()),
            )
        },
        None => (
            HttpClientWithContext::external(withheld_context(), HashMap::new(), limits)?,
            config,
        ),
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
