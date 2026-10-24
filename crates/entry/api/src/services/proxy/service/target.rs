//! Target resolution for a proxied request: the external-server lookup and
//! the unknown-service challenge for MCP.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::McpServerConfig;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use super::ProxyKind;
use crate::services::proxy::auth::build_mcp_unknown_service_challenge;
use crate::services::proxy::backend::ProxyError;

pub(super) fn unknown_service_error(
    service_name: &ServiceName,
    proxy_kind: ProxyKind,
    request: &Request<Body>,
    ctx: &AppContext,
    err: ProxyError,
) -> ProxyError {
    if proxy_kind == ProxyKind::Mcp && matches!(err, ProxyError::ServiceNotFound { .. }) {
        let req_ctx = request.extensions().get::<RequestContext>().cloned();
        if let Some(challenge) = build_mcp_unknown_service_challenge(
            service_name,
            request.headers(),
            ctx,
            req_ctx.as_ref(),
        ) {
            return challenge;
        }
    }
    err
}

pub(super) fn external_server(
    proxy_kind: ProxyKind,
    service_name: &ServiceName,
    ctx: &AppContext,
) -> Result<Option<McpServerConfig>, ProxyError> {
    if !matches!(proxy_kind, ProxyKind::Mcp) {
        return Ok(None);
    }
    let found = ctx
        .mcp_registry()
        .find_server(service_name.as_str())
        .map_err(|source| ProxyError::RegistryUnavailable {
            service: service_name.to_string(),
            source,
        })?;
    Ok(found.filter(McpServerConfig::is_external))
}
