//! Proxy-side identity store for MCP sessions.
//!
//! MCP clients authenticate on the `initialize` call but may omit the bearer
//! token on subsequent session-only requests. This module persists the
//! authenticated identity keyed by `mcp-session-id` so those follow-ups can be
//! enriched ([`enrich_with_cached_identity`]) on any replica, and evicts the
//! row on session teardown or a stale-session backend response
//! ([`handle_mcp_response`]). The store is the trust anchor for session-based
//! MCP auth — rows are only written for a verified [`AuthenticatedUser`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_mcp::repository::{McpProxyIdentityRepository, ProxyIdentityRow};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;
use uuid::Uuid;

fn session_id_header(headers: &HeaderMap) -> Option<SessionId> {
    headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| SessionId::new(s.to_owned()))
}

async fn evict(identities: &McpProxyIdentityRepository, session_id: &SessionId, reason: &str) {
    if let Err(e) = identities.delete(session_id).await {
        tracing::warn!(session_id = %session_id, error = %e, reason, "Failed to evict proxy session identity");
    }
}

pub(super) async fn enrich_with_cached_identity(
    identities: &McpProxyIdentityRepository,
    request_headers: &HeaderMap,
    req_context: RequestContext,
    service_name: &str,
) -> RequestContext {
    let Some(session_id) = session_id_header(request_headers) else {
        return req_context;
    };

    let identity = match identities.find(&session_id).await {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            tracing::debug!(
                service = %service_name,
                session_id = %session_id,
                "No stored identity for session-only MCP request"
            );
            return req_context;
        },
        Err(e) => {
            tracing::warn!(
                service = %service_name,
                session_id = %session_id,
                error = %e,
                "Proxy session identity lookup failed"
            );
            return req_context;
        },
    };

    let Ok(user_uuid) = Uuid::parse_str(identity.user_id.as_str()) else {
        tracing::warn!(
            service = %service_name,
            session_id = %session_id,
            user_id = %identity.user_id,
            "Stored proxy session identity has a non-UUID user id"
        );
        return req_context;
    };

    tracing::info!(
        service = %service_name,
        session_id = %session_id,
        user_id = %identity.user_id,
        "Enriching session-only request with stored identity"
    );
    req_context
        .with_actor(systemprompt_identifiers::Actor::user(identity.user_id))
        .with_user_type(identity.user_type)
        .with_auth_token(identity.auth_token.as_str().to_owned())
        .with_user(AuthenticatedUser::new(
            user_uuid,
            String::new(),
            String::new(),
            identity.permissions,
        ))
}

pub(super) struct McpResponseCtx<'a> {
    pub identities: &'a McpProxyIdentityRepository,
    pub response: &'a reqwest::Response,
    pub request_headers: &'a HeaderMap,
    pub req_context: &'a RequestContext,
    pub authenticated_user: Option<&'a AuthenticatedUser>,
    pub service_name: &'a str,
    pub method_str: &'a str,
}

pub(super) async fn handle_mcp_response(args: McpResponseCtx<'_>) {
    let McpResponseCtx {
        identities,
        response,
        request_headers,
        req_context,
        authenticated_user,
        service_name,
        method_str,
    } = args;
    let resp_status = response.status();
    let resp_session = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none");
    let resp_content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none");

    tracing::info!(
        service = %service_name,
        status = %resp_status,
        resp_session_id = %resp_session,
        content_type = %resp_content_type,
        method = %method_str,
        "MCP backend response"
    );

    if !resp_status.is_success() {
        evict_on_error_response(
            identities,
            response,
            request_headers,
            service_name,
            method_str,
        )
        .await;
    }

    cache_identity_from_response(
        identities,
        response,
        req_context,
        authenticated_user,
        service_name,
    )
    .await;

    if method_str == "DELETE"
        && let Some(session_id) = session_id_header(request_headers)
    {
        evict(identities, &session_id, "delete").await;
        tracing::debug!(session_id = %session_id, "Evicted session identity on DELETE");
    }
}

async fn evict_on_error_response(
    identities: &McpProxyIdentityRepository,
    response: &reqwest::Response,
    request_headers: &HeaderMap,
    service_name: &str,
    method_str: &str,
) {
    let resp_status = response.status();
    let header_dump: Vec<String> = response
        .headers()
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("?")))
        .collect();
    tracing::error!(
        service = %service_name,
        status = %resp_status,
        headers = ?header_dump,
        "MCP backend error response"
    );

    if resp_status == StatusCode::NOT_FOUND
        && method_str == "GET"
        && let Some(session_id) = session_id_header(request_headers)
    {
        evict(identities, &session_id, "stale_session").await;
        tracing::info!(
            service = %service_name,
            session_id = %session_id,
            "Evicted stale proxy session identity on 404 GET"
        );
    }
}

async fn cache_identity_from_response(
    identities: &McpProxyIdentityRepository,
    response: &reqwest::Response,
    req_context: &RequestContext,
    authenticated_user: Option<&AuthenticatedUser>,
    service_name: &str,
) {
    let Some(session_id) = session_id_header(response.headers()) else {
        return;
    };
    let Some(user) = authenticated_user else {
        return;
    };
    let row = ProxyIdentityRow {
        user_id: UserId::new(user.id.to_string()),
        user_type: req_context.user_type(),
        permissions: user.permissions.clone(),
        auth_token: req_context.auth_token().clone(),
    };
    match identities.upsert(&session_id, &row).await {
        Ok(()) => tracing::info!(
            service = %service_name,
            session_id = %session_id,
            user_id = %user.id,
            "Stored session identity for MCP session"
        ),
        Err(e) => tracing::error!(
            service = %service_name,
            session_id = %session_id,
            user_id = %user.id,
            error = %e,
            "Failed to store session identity for MCP session"
        ),
    }
}
