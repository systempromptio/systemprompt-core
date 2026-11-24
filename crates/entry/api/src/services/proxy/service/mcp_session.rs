//! Managed MCP session binding at the proxy edge.
//!
//! A managed MCP server issues a session id on `initialize`; the proxy binds
//! it to the server and the principal that opened it through the MCP
//! backends, and refuses a later request naming a session that is not bound
//! to the same pair (404, so the client initializes again).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_extension::router::ProxyBackendResolver;
use systemprompt_identifiers::{ServiceName, SessionId, UserId};

use super::super::backend::ProxyError;

pub const SESSION_HEADER: &str = systemprompt_identifiers::headers::MCP_SESSION_ID;

pub(crate) fn session_id_header(headers: &HeaderMap) -> Option<SessionId> {
    headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(SessionId::new)
}

pub(crate) async fn ensure_bound(
    backends: &dyn ProxyBackendResolver,
    request_headers: &HeaderMap,
    service_name: &ServiceName,
    principal: Option<&UserId>,
) -> Result<(), ProxyError> {
    if request_headers.get(SESSION_HEADER).is_none() {
        return Ok(());
    }
    let not_found = || ProxyError::SessionNotFound {
        service: service_name.to_string(),
    };
    let session_id = session_id_header(request_headers).ok_or_else(not_found)?;
    let bound = backends
        .session_bound(service_name, &session_id, principal)
        .await
        .map_err(|error| ProxyError::from_backend(service_name, error))?;
    if bound {
        return Ok(());
    }
    tracing::debug!(
        service = %service_name,
        session_id = %session_id,
        "MCP session is not bound to this server and principal"
    );
    Err(not_found())
}

#[derive(Debug)]
pub struct IssuedSession<'a> {
    pub backends: &'a dyn ProxyBackendResolver,
    pub response: &'a reqwest::Response,
    pub request_headers: &'a HeaderMap,
    pub service_name: &'a ServiceName,
    pub principal: Option<&'a UserId>,
    pub method_str: &'a str,
}

pub(crate) async fn bind_issued_session(issued: IssuedSession<'_>) -> Result<(), ProxyError> {
    let IssuedSession {
        backends,
        response,
        request_headers,
        service_name,
        principal,
        method_str,
    } = issued;
    let status = response.status();
    if status.is_success() {
        tracing::debug!(service = %service_name, %status, method = %method_str, "MCP backend response");
    } else {
        tracing::warn!(service = %service_name, %status, method = %method_str, "MCP backend error response");
    }
    if !status.is_success() || request_headers.get(SESSION_HEADER).is_some() {
        return Ok(());
    }
    let Some(session_id) = session_id_header(response.headers()) else {
        return Ok(());
    };
    backends
        .bind_session(service_name, &session_id, principal)
        .await
        .map_err(|error| ProxyError::from_backend(service_name, error))?;
    tracing::debug!(
        service = %service_name,
        session_id = %session_id,
        "Bound MCP session to its server and principal"
    );
    Ok(())
}
