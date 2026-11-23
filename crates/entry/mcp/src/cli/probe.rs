//! The connection `plugins mcp tools` and `plugins mcp call` make to an MCP
//! server.
//!
//! Every connection goes through the API's MCP proxy at
//! `<api_external_url>/api/v1/mcp/<server>/mcp` and presents the caller's own
//! session. The session token is a platform credential and reaches only the
//! API: the proxy verifies it, attributes the request to the token's user and
//! session, checks the forwarded context id against that user, injects an
//! external server's per-user bearer, and audits the call as it audits any
//! caller. No command connects to a server's loopback port or presents the
//! session to an external endpoint. When nothing answers at the API's
//! address the connection fails with [`McpCallError::ApiUnreachable`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::model::{
    ClientCapabilities, ClientConfig, Implementation, PaginatedRequestParams, ProtocolVersion,
};
use rmcp::service::{ClientInitializeError, RoleClient, RunningService, ServiceError};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{AgentName, McpServerId};
use systemprompt_manifest::services::{McpConfig, ResilienceSettings};
use systemprompt_mcp::services::client::{
    HttpClientWithContext, McpClientLimits, McpTransportError,
};
use systemprompt_models::modules::ApiPaths;
use tokio::time::timeout;

use super::types::ToolInfo;

#[derive(Debug, thiserror::Error)]
pub enum McpCallError {
    #[error(
        "the API at {url} is not reachable; MCP servers are reached only through its proxy, so \
         start it (`infra services start`) and retry"
    )]
    ApiUnreachable {
        url: String,
        #[source]
        source: Box<ClientInitializeError>,
    },
    #[error("failed to connect to MCP server '{server}' through the API proxy at {url}")]
    Connect {
        server: McpServerId,
        url: String,
        #[source]
        source: Box<ClientInitializeError>,
    },
    #[error("MCP server name '{server}' does not form a valid agent name")]
    AgentName {
        server: McpServerId,
        #[source]
        source: IdValidationError,
    },
    #[error("the MCP client for '{server}' could not be built")]
    Client {
        server: McpServerId,
        #[source]
        source: Box<McpTransportError>,
    },
    #[error("listing tools on '{server}' failed")]
    ListTools {
        server: McpServerId,
        #[source]
        source: Box<ServiceError>,
    },
    #[error("listing tools on '{server}' did not finish within {}s", limit.as_secs())]
    Timeout {
        server: McpServerId,
        limit: Duration,
    },
}

#[derive(Debug)]
pub struct McpTarget<'a> {
    server_name: McpServerId,
    url: String,
    session: &'a CallerSession,
}

impl<'a> McpTarget<'a> {
    #[must_use]
    pub fn through_proxy(
        api_external_url: &str,
        server_name: McpServerId,
        session: &'a CallerSession,
    ) -> Self {
        let url = proxy_url(api_external_url, &server_name);
        Self {
            server_name,
            url,
            session,
        }
    }

    #[must_use]
    pub const fn server_name(&self) -> &McpServerId {
        &self.server_name
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[must_use]
pub fn proxy_url(api_external_url: &str, server: &McpServerId) -> String {
    format!(
        "{}{}",
        api_external_url.trim_end_matches('/'),
        ApiPaths::mcp_server_endpoint(server.as_str())
    )
}

#[must_use]
pub fn cli_limits(timeout_secs: u64) -> McpClientLimits {
    McpClientLimits::from(&ResilienceSettings {
        request_timeout_ms: timeout_secs.saturating_mul(1000),
        ..McpConfig::default().resilience
    })
}

pub async fn connect(
    target: &McpTarget<'_>,
    timeout_secs: u64,
) -> Result<RunningService<RoleClient, ClientConfig>, McpCallError> {
    let server = target.server_name();
    let agent_name =
        AgentName::try_new(format!("cli-{server}")).map_err(|source| McpCallError::AgentName {
            server: server.clone(),
            source,
        })?;
    let http_client = HttpClientWithContext::forwarding(
        target.session.request_context(agent_name),
        HashMap::new(),
        cli_limits(timeout_secs),
    )
    .map_err(|source| McpCallError::Client {
        server: server.clone(),
        source: Box::new(source),
    })?;
    let config = StreamableHttpClientTransportConfig::with_uri(target.url.clone())
        .auth_header(target.session.token().as_str().to_owned());
    let transport = StreamableHttpClientTransport::with_client(http_client, config);

    let client_info = ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new(format!("systemprompt-cli-{server}"), "1.0.0"),
    )
    .with_protocol_version(ProtocolVersion::LATEST_WITH_INITIALIZE);

    match client_info.serve(transport).await {
        Ok(client) => Ok(client),
        Err(source) => Err(if api_answers(&target.url).await {
            McpCallError::Connect {
                server: server.clone(),
                url: target.url.clone(),
                source: Box::new(source),
            }
        } else {
            McpCallError::ApiUnreachable {
                url: target.url.clone(),
                source: Box::new(source),
            }
        }),
    }
}

async fn api_answers(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    let address = if authority
        .rsplit_once(':')
        .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
    {
        authority.to_owned()
    } else {
        let port = if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        };
        format!("{authority}:{port}")
    };
    matches!(
        timeout(API_PROBE_TIMEOUT, tokio::net::TcpStream::connect(address)).await,
        Ok(Ok(_))
    )
}

const API_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn list_tools(
    target: &McpTarget<'_>,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>, McpCallError> {
    let limit = Duration::from_secs(timeout_secs);
    timeout(limit, fetch_tools(target, timeout_secs))
        .await
        .map_err(|_elapsed| McpCallError::Timeout {
            server: target.server_name().clone(),
            limit,
        })?
}

async fn fetch_tools(
    target: &McpTarget<'_>,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>, McpCallError> {
    let client = connect(target, timeout_secs).await?;
    let listed = client
        // Why: rmcp serialises a `None` params as `"params": null`, which a
        // strict server (Google's MCP) refuses with -32602; send `{}`.
        .list_tools(Some(PaginatedRequestParams::default()))
        .await
        .map_err(|source| McpCallError::ListTools {
            server: target.server_name().clone(),
            source: Box::new(source),
        });
    if let Err(error) = client.cancel().await {
        tracing::debug!(server = %target.server_name(), error = %error, "MCP client did not close cleanly");
    }
    Ok(listed?.tools.into_iter().map(ToolInfo::from_tool).collect())
}
