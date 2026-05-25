//! Proxy-verified-identity short-circuit for trusted upstream gateways.

use rmcp::service::RequestContext as McpContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;

use super::jwt::validate_scopes_for_permissions;
use super::{AuthResult, AuthenticatedRequestContext};

pub(super) fn try_proxy_verified_auth(
    mcp_context: &McpContext<RoleServer>,
    request_context: RequestContext,
    oauth_config: &crate::OAuthRequirement,
    server_name: &str,
) -> Result<Option<AuthResult>, McpError> {
    let parts = mcp_context
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| {
            McpError::invalid_request("No HTTP parts in MCP context".to_owned(), None)
        })?;

    let proxy_verified = parts
        .headers
        .get("x-proxy-verified")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "true");

    if !proxy_verified {
        return Ok(None);
    }

    let user_id_str = parts
        .headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            McpError::invalid_request(
                "Proxy-verified request missing x-user-id header".to_owned(),
                None,
            )
        })?;

    let permissions = parts
        .headers
        .get("x-user-permissions")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| systemprompt_models::auth::parse_permissions(s).ok())
        .ok_or_else(|| {
            McpError::invalid_request(
                "Proxy-verified request missing x-user-permissions header".to_owned(),
                None,
            )
        })?;

    validate_scopes_for_permissions(server_name, &permissions, oauth_config)?;

    let user_id: uuid::Uuid = user_id_str.parse().map_err(|e| {
        McpError::invalid_request(format!("Invalid user ID in x-user-id header: {e}"), None)
    })?;
    let authenticated_user =
        AuthenticatedUser::new(user_id, String::new(), String::new(), permissions);

    let token = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            McpError::invalid_request(
                "Proxy-verified request missing Authorization Bearer token".to_owned(),
                None,
            )
        })?.to_owned();

    let context = request_context
        .with_user(authenticated_user)
        .with_actor(Actor::user(UserId::new(user_id_str.to_owned())));

    tracing::info!(
        server = %server_name,
        user_id = %user_id_str,
        "Authorized via proxy-verified identity"
    );

    Ok(Some(AuthResult::Authenticated(
        AuthenticatedRequestContext::new(context, token),
    )))
}
