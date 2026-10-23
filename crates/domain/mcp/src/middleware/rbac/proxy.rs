//! Proxy-verified identity for trusted upstream gateways. The identity
//! comes from the gateway's headers, but the per-server authz hook still
//! runs over it — proxy verification replaces JWT parsing, never policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use rmcp::ErrorData as McpError;
use systemprompt_governance::authz::{AuthzContext, AuthzRequest, EntityRef};
use systemprompt_identifiers::{Actor, JwtToken, McpServerId, UserId, headers};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;

use super::AuthenticatedRequestContext;
use super::jwt::validate_scopes_for_permissions;

pub fn try_proxy_verified_auth(
    parts: Option<&http::request::Parts>,
    request_context: RequestContext,
    oauth_config: &crate::OAuthRequirement,
    server_id: &McpServerId,
) -> Result<Option<AuthenticatedRequestContext>, McpError> {
    let parts = parts.ok_or_else(|| {
        McpError::invalid_request("No HTTP parts in MCP context".to_owned(), None)
    })?;

    if header_str(parts, headers::PROXY_VERIFIED) != Some("true") {
        return Ok(None);
    }

    let user_id_str = header_str(parts, headers::USER_ID)
        .ok_or_else(|| missing_header("Proxy-verified request missing x-user-id header"))?;
    let permissions = proxied_permissions(parts)?;
    validate_scopes_for_permissions(server_id, &permissions, oauth_config)?;

    let user_id = UserId::try_new(user_id_str).map_err(|error| {
        tracing::warn!(%error, "Rejected proxy-verified request with a malformed x-user-id");
        McpError::invalid_request("Invalid user ID in x-user-id header".to_owned(), None)
    })?;
    let authenticated_user = AuthenticatedUser::new_with_roles(
        user_id.clone(),
        String::new(),
        String::new(),
        permissions,
        proxied_roles(parts),
    );
    let token = proxied_bearer(parts)?;

    let context = request_context
        .with_user(authenticated_user)
        .with_actor(Actor::user(user_id));

    tracing::info!(
        server = %server_id,
        user_id = %user_id_str,
        "Authorized via proxy-verified identity"
    );

    Ok(Some(AuthenticatedRequestContext::new(context, token)))
}

fn header_str<'a>(parts: &'a http::request::Parts, name: &str) -> Option<&'a str> {
    parts.headers.get(name).and_then(|v| v.to_str().ok())
}

fn missing_header(message: &str) -> McpError {
    McpError::invalid_request(message.to_owned(), None)
}

fn proxied_permissions(
    parts: &http::request::Parts,
) -> Result<Vec<systemprompt_models::auth::Permission>, McpError> {
    header_str(parts, headers::USER_PERMISSIONS)
        .and_then(|s| systemprompt_models::auth::parse_permissions(s).ok())
        .ok_or_else(|| missing_header("Proxy-verified request missing x-user-permissions header"))
}

// Why: a gateway that did not decorate the hop with roles asserted none;
// the subject is evaluated with exactly the roles it presented.
fn proxied_roles(parts: &http::request::Parts) -> Vec<String> {
    header_str(parts, headers::USER_ROLES)
        .map(systemprompt_models::auth::parse_roles)
        .unwrap_or_default()
}

fn proxied_bearer(parts: &http::request::Parts) -> Result<JwtToken, McpError> {
    header_str(parts, headers::AUTHORIZATION)
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(JwtToken::new)
        .ok_or_else(|| missing_header("Proxy-verified request missing Authorization Bearer token"))
}

// JSON: marketplace policy floor — open attribute map passed to the authz
// context.
#[must_use]
pub(super) fn build_proxy_authz_request(
    server_id: &McpServerId,
    context: &RequestContext,
    floor: Option<&BTreeMap<String, serde_json::Value>>,
) -> AuthzRequest {
    let authz_context = floor.map_or_else(AuthzContext::none, |floor| {
        AuthzContext::none().with_marketplace_floor(floor)
    });
    let user_id = context.user_id().clone();
    let (roles, attributes) = context.user.as_ref().map_or_else(
        || (Vec::new(), BTreeMap::new()),
        |user| (user.roles.clone(), user.attributes.clone()),
    );
    AuthzRequest {
        entity: EntityRef::McpServer(server_id.clone()),
        user_id: user_id.clone(),
        actor: Some(Actor::mcp(user_id, server_id.as_str())),
        client_id: context.client_id().cloned(),
        access_scope: None,
        roles,
        attributes,
        trace_id: context.trace_id().clone(),
        session_id: Some(context.session_id().clone()),
        context: authz_context,
        context_id: Some(context.context_id().clone()),
        task_id: context.task_id().cloned(),
        act_chain: context.act_chain().to_vec(),
    }
}
