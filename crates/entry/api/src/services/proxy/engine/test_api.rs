//! Test-only seams over the proxy session cache and external MCP helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use systemprompt_identifiers::{JwtToken, SessionId};
use systemprompt_mcp::McpDomainError;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission, UserType};
use uuid::Uuid;

use super::mcp_session::{McpResponseCtx, ProxySessionIdentity, SessionCache};

#[derive(Clone, Debug, Default)]
pub struct TestSessionCache(SessionCache);

impl TestSessionCache {
    pub async fn seed(
        &self,
        session_id: &SessionId,
        user: Uuid,
        permissions: Vec<Permission>,
        token: &str,
    ) {
        self.0.write().await.insert(
            session_id.as_str().to_owned(),
            ProxySessionIdentity {
                user,
                user_type: UserType::User,
                permissions,
                auth_token: JwtToken::new(token),
            },
        );
    }

    pub async fn cached_user(&self, session_id: &SessionId) -> Option<Uuid> {
        self.0.read().await.get(session_id.as_str()).map(|i| i.user)
    }
}

pub async fn enrich_with_cached_identity(
    cache: &TestSessionCache,
    request_headers: &HeaderMap,
    req_context: RequestContext,
    service_name: &str,
) -> RequestContext {
    super::mcp_session::enrich_with_cached_identity(
        &cache.0,
        request_headers,
        req_context,
        service_name,
    )
    .await
}

#[derive(Debug)]
pub struct ResponseArgs<'a> {
    pub cache: &'a TestSessionCache,
    pub response: &'a reqwest::Response,
    pub request_headers: &'a HeaderMap,
    pub req_context: &'a RequestContext,
    pub authenticated_user: Option<&'a AuthenticatedUser>,
    pub service_name: &'a str,
    pub method_str: &'a str,
}

pub async fn handle_mcp_response(args: ResponseArgs<'_>) {
    super::mcp_session::handle_mcp_response(McpResponseCtx {
        cache: &args.cache.0,
        response: args.response,
        request_headers: args.request_headers,
        req_context: args.req_context,
        authenticated_user: args.authenticated_user,
        service_name: args.service_name,
        method_str: args.method_str,
    })
    .await;
}

#[must_use]
pub fn outbound_headers(
    incoming: &HeaderMap,
    provider: Vec<(HeaderName, HeaderValue)>,
) -> HeaderMap {
    super::external::outbound_headers(incoming, provider.into_iter().collect::<HashMap<_, _>>())
}

#[must_use]
pub fn map_resolve_error(service_name: &str, error: McpDomainError) -> String {
    super::external::map_resolve_error(service_name, error).to_string()
}
