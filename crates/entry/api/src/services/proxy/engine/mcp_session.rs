//! Proxy-side identity cache for MCP sessions.
//!
//! MCP clients authenticate on the `initialize` call but may omit the bearer
//! token on subsequent session-only requests. This module caches the
//! authenticated identity keyed by `mcp-session-id` so those follow-ups can be
//! enriched ([`enrich_with_cached_identity`]), and evicts the entry on session
//! teardown or a stale-session backend response ([`handle_mcp_response`]). The
//! cache is the trust anchor for session-based MCP auth — entries are only
//! written for a verified [`AuthenticatedUser`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission, UserType};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) struct ProxySessionIdentity {
    pub user: Uuid,
    pub user_type: UserType,
    pub permissions: Vec<Permission>,
    pub auth_token: JwtToken,
}

pub(super) type SessionCache = Arc<RwLock<HashMap<String, ProxySessionIdentity>>>;

pub(super) async fn enrich_with_cached_identity(
    cache: &SessionCache,
    request_headers: &HeaderMap,
    mut req_context: RequestContext,
    service_name: &str,
) -> RequestContext {
    let Some(session_id) = request_headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
    else {
        return req_context;
    };

    if let Some(identity) = cache.read().await.get(session_id) {
        tracing::info!(
            service = %service_name,
            session_id = %session_id,
            user_id = %identity.user,
            "Enriching session-only request with cached identity"
        );
        req_context = req_context
            .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
                identity.user.to_string(),
            )))
            .with_user_type(identity.user_type)
            .with_auth_token(identity.auth_token.as_str().to_owned())
            .with_user(AuthenticatedUser::new(
                identity.user,
                String::new(),
                String::new(),
                identity.permissions.clone(),
            ));
    }
    req_context
}

pub(super) struct McpResponseCtx<'a> {
    pub cache: &'a SessionCache,
    pub response: &'a reqwest::Response,
    pub request_headers: &'a HeaderMap,
    pub req_context: &'a RequestContext,
    pub authenticated_user: Option<&'a AuthenticatedUser>,
    pub service_name: &'a str,
    pub method_str: &'a str,
}

pub(super) async fn handle_mcp_response(args: McpResponseCtx<'_>) {
    let McpResponseCtx {
        cache,
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
        evict_on_error_response(cache, response, request_headers, service_name, method_str).await;
    }

    cache_identity_from_response(
        cache,
        response,
        req_context,
        authenticated_user,
        service_name,
    )
    .await;

    if method_str == "DELETE"
        && let Some(session_id) = request_headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
    {
        cache.write().await.remove(session_id);
        tracing::debug!(session_id = %session_id, "Evicted session identity on DELETE");
    }
}

async fn evict_on_error_response(
    cache: &SessionCache,
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
        && let Some(session_id) = request_headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
    {
        cache.write().await.remove(session_id);
        tracing::info!(
            service = %service_name,
            session_id = %session_id,
            "Evicted stale proxy session cache on 404 GET"
        );
    }
}

async fn cache_identity_from_response(
    cache: &SessionCache,
    response: &reqwest::Response,
    req_context: &RequestContext,
    authenticated_user: Option<&AuthenticatedUser>,
    service_name: &str,
) {
    let Some(session_id) = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
    else {
        return;
    };
    let Some(user) = authenticated_user else {
        return;
    };
    cache.write().await.insert(
        session_id.to_owned(),
        ProxySessionIdentity {
            user: user.id,
            user_type: req_context.user_type(),
            permissions: user.permissions.clone(),
            auth_token: req_context.auth_token().clone(),
        },
    );
    tracing::info!(
        service = %service_name,
        session_id = %session_id,
        user_id = %user.id,
        "Cached session identity for MCP session"
    );
}
