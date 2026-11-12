//! Binding of managed MCP sessions to the server and principal that opened
//! them.
//!
//! A session id is a correlation handle, never a credential: every request is
//! authenticated on its own credential at the edge, and the session id only
//! selects which backend session the request belongs to. The `initialize`
//! response that mints a session binds it to `(server, principal)`
//! ([`bind_issued_session`]); every later request naming a session must come
//! from the same principal on the same server ([`ensure_bound`]) or it is
//! answered 404, exactly as for a session that does not exist. Session ids
//! are not written to logs above debug level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::{McpServerId, ServiceName, SessionId, UserId};
use systemprompt_mcp::repository::McpProxySessionRepository;

use super::super::backend::ProxyError;

pub const SESSION_HEADER: &str = "mcp-session-id";

pub fn session_id_header(headers: &HeaderMap) -> Option<SessionId> {
    headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(SessionId::new)
}

pub async fn ensure_bound(
    sessions: &McpProxySessionRepository,
    request_headers: &HeaderMap,
    service_name: &ServiceName,
    principal: Option<&UserId>,
) -> Result<(), ProxyError> {
    if request_headers.get(SESSION_HEADER).is_none() {
        return Ok(());
    }
    let Some(session_id) = session_id_header(request_headers) else {
        return Err(ProxyError::SessionNotFound {
            service: service_name.to_string(),
        });
    };
    let bound = sessions
        .is_bound_to(
            &session_id,
            &McpServerId::new(service_name.as_str()),
            principal,
        )
        .await
        .map_err(|source| ProxyError::SessionLookupFailed {
            service: service_name.to_string(),
            source,
        })?;
    if bound {
        return Ok(());
    }
    tracing::debug!(
        service = %service_name,
        session_id = %session_id,
        "MCP session is not bound to this server and principal"
    );
    Err(ProxyError::SessionNotFound {
        service: service_name.to_string(),
    })
}

#[derive(Debug)]
pub struct IssuedSession<'a> {
    pub sessions: &'a McpProxySessionRepository,
    pub response: &'a reqwest::Response,
    pub request_headers: &'a HeaderMap,
    pub service_name: &'a ServiceName,
    pub principal: Option<&'a UserId>,
    pub method_str: &'a str,
}

pub async fn bind_issued_session(issued: IssuedSession<'_>) -> Result<(), ProxyError> {
    let IssuedSession {
        sessions,
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
    sessions
        .bind(
            &session_id,
            &McpServerId::new(service_name.as_str()),
            principal,
        )
        .await
        .map_err(|source| ProxyError::SessionLookupFailed {
            service: service_name.to_string(),
            source,
        })?;
    tracing::debug!(
        service = %service_name,
        session_id = %session_id,
        "Bound MCP session to its server and principal"
    );
    Ok(())
}
