//! Test-side wrappers over the proxy MCP session-identity store, seeding and
//! reading rows directly so the suites can assert on cache state.

use axum::http::HeaderMap;
use systemprompt_api::services::proxy::engine::mcp_session::{self, McpResponseCtx};
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_mcp::repository::{McpProxyIdentityRepository, ProxyIdentityRow};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission, UserType};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TestSessionCache(McpProxyIdentityRepository);

impl TestSessionCache {
    pub const fn new(identities: McpProxyIdentityRepository) -> Self {
        Self(identities)
    }

    pub async fn seed(
        &self,
        session_id: &SessionId,
        user: Uuid,
        permissions: Vec<Permission>,
        token: &str,
    ) {
        self.0
            .upsert(
                session_id,
                &ProxyIdentityRow {
                    user_id: UserId::new(user.to_string()),
                    user_type: UserType::User,
                    permissions,
                    auth_token: JwtToken::new(token),
                },
            )
            .await
            .expect("seed proxy identity");
    }

    pub async fn cached_user(&self, session_id: &SessionId) -> Option<Uuid> {
        self.0
            .find(session_id)
            .await
            .expect("find proxy identity")
            .and_then(|row| Uuid::parse_str(row.user_id.as_str()).ok())
    }
}

pub async fn enrich_with_cached_identity(
    cache: &TestSessionCache,
    request_headers: &HeaderMap,
    req_context: RequestContext,
    service_name: &str,
) -> RequestContext {
    mcp_session::enrich_with_cached_identity(&cache.0, request_headers, req_context, service_name)
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
    mcp_session::handle_mcp_response(McpResponseCtx {
        identities: &args.cache.0,
        response: args.response,
        request_headers: args.request_headers,
        req_context: args.req_context,
        authenticated_user: args.authenticated_user,
        service_name: args.service_name,
        method_str: args.method_str,
    })
    .await;
}
