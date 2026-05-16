use axum::http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub(super) struct ProxySessionIdentity {
    pub user: String,
    pub user_type: String,
    pub permissions: Vec<Permission>,
    pub auth_token: String,
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
        let user_type = identity.user_type.parse().unwrap_or_else(|_| {
            tracing::warn!(
                user_type = %identity.user_type,
                "Unrecognised cached user type, defaulting to Unknown"
            );
            systemprompt_models::auth::UserType::Unknown
        });
        let user_uuid = identity.user.parse().unwrap_or_else(|_| {
            tracing::warn!(user = %identity.user, "Cached user id is not a valid UUID");
            uuid::Uuid::nil()
        });
        req_context = req_context
            .with_user_id(UserId::new(identity.user.clone()))
            .with_user_type(user_type)
            .with_auth_token(identity.auth_token.clone())
            .with_user(AuthenticatedUser::new(
                user_uuid,
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

        if resp_status == StatusCode::NOT_FOUND && method_str == "GET" {
            if let Some(session_id) = request_headers
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
    }

    if let Some(session_id) = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(user) = authenticated_user {
            cache.write().await.insert(
                session_id.to_string(),
                ProxySessionIdentity {
                    user: user.id.to_string(),
                    user_type: req_context.user_type().to_string(),
                    permissions: user.permissions.clone(),
                    auth_token: req_context.auth_token().as_str().to_string(),
                },
            );
            tracing::info!(
                service = %service_name,
                session_id = %session_id,
                user_id = %user.id,
                "Cached session identity for MCP session"
            );
        }
    }

    if method_str == "DELETE" {
        if let Some(session_id) = request_headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            cache.write().await.remove(session_id);
            tracing::debug!(session_id = %session_id, "Evicted session identity on DELETE");
        }
    }
}
