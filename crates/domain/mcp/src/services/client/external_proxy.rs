//! Resolve the forward target for an external MCP server served over HTTP.
//!
//! The HTTP gateway proxies a per-user MCP client (the bridge) to an external
//! provider without terminating the MCP protocol. It cannot reach the
//! `pub(super)` bearer-minting helpers directly, so this seam resolves the
//! provider URL and the per-user outbound headers in one call: the provider
//! bearer is minted server-side from the caller's systemprompt JWT and the
//! systemprompt credential is withheld from the provider.

use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::McpServerConfig;

use super::McpClient;
use crate::error::{McpDomainError, McpDomainResult};

/// Forward target for an external MCP request.
///
/// `headers` carry the per-user provider bearer; `url` is the provider endpoint
/// and must never be surfaced to the client.
#[derive(Debug)]
pub struct ExternalProxyTarget {
    pub url: String,
    pub headers: HashMap<HeaderName, HeaderValue>,
}

impl McpClient {
    pub async fn resolve_external_proxy_target(
        server_config: &McpServerConfig,
        context: &RequestContext,
    ) -> McpDomainResult<ExternalProxyTarget> {
        let ext = server_config.external_auth.as_ref().ok_or_else(|| {
            McpDomainError::Configuration(format!(
                "MCP server '{}' has no external_auth accessor to mint a per-user bearer",
                server_config.name
            ))
        })?;

        let bearer =
            super::external_auth::resolve_external_bearer(ext, context, &server_config.name)
                .await?;
        let headers = super::external_auth::outbound_headers(
            ext,
            &bearer,
            &server_config.headers,
            &server_config.name,
        )?;

        Ok(ExternalProxyTarget {
            url: server_config.remote_endpoint.clone(),
            headers,
        })
    }
}
