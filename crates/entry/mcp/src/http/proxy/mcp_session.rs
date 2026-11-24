//! The managed MCP session binding: a session a managed server issues is
//! bound to that server and the principal that opened it, and a later
//! request naming the session is forwarded only for the same pair.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::router::ProxyBackendError;
use systemprompt_identifiers::{McpServerId, ServiceName, SessionId, UserId};
use systemprompt_mcp::repository::McpProxySessionRepository;

pub(super) async fn is_bound(
    sessions: &McpProxySessionRepository,
    service: &ServiceName,
    session: &SessionId,
    principal: Option<&UserId>,
) -> Result<bool, ProxyBackendError> {
    sessions
        .is_bound_to(session, &McpServerId::new(service.as_str()), principal)
        .await
        .map_err(|source| ProxyBackendError::SessionLookupFailed(Box::new(source)))
}

pub(super) async fn bind(
    sessions: &McpProxySessionRepository,
    service: &ServiceName,
    session: &SessionId,
    principal: Option<&UserId>,
) -> Result<(), ProxyBackendError> {
    sessions
        .bind(session, &McpServerId::new(service.as_str()), principal)
        .await
        .map_err(|source| ProxyBackendError::SessionLookupFailed(Box::new(source)))
}
