//! Proxy-verified identity for trusted upstream gateways. The identity
//! comes from the gateway's headers, but the per-server authz hook still
//! runs over it — proxy verification replaces JWT parsing, never policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{Actor, McpServerId, UserId, headers};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_security::authz::{AuthzContext, AuthzRequest, EntityRef};

use super::AuthenticatedRequestContext;
use super::jwt::validate_scopes_for_permissions;

pub fn try_proxy_verified_auth(
    parts: Option<&http::request::Parts>,
    request_context: RequestContext,
    oauth_config: &crate::OAuthRequirement,
    server_name: &str,
) -> Result<Option<AuthenticatedRequestContext>, McpError> {
    let parts = parts.ok_or_else(|| {
        McpError::invalid_request("No HTTP parts in MCP context".to_owned(), None)
    })?;

    let proxy_verified = parts
        .headers
        .get(headers::PROXY_VERIFIED)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "true");

    if !proxy_verified {
        return Ok(None);
    }

    let user_id_str = parts
        .headers
        .get(headers::USER_ID)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            McpError::invalid_request(
                "Proxy-verified request missing x-user-id header".to_owned(),
                None,
            )
        })?;

    let permissions = parts
        .headers
        .get(headers::USER_PERMISSIONS)
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
    // Why: a gateway that did not decorate the hop with roles asserted none;
    // the subject is evaluated with exactly the roles it presented.
    let roles = parts
        .headers
        .get(headers::USER_ROLES)
        .and_then(|v| v.to_str().ok())
        .map(systemprompt_models::auth::parse_roles)
        .unwrap_or_default();
    let authenticated_user = AuthenticatedUser::new_with_roles(
        user_id,
        String::new(),
        String::new(),
        permissions,
        roles,
    );

    let token = parts
        .headers
        .get(headers::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            McpError::invalid_request(
                "Proxy-verified request missing Authorization Bearer token".to_owned(),
                None,
            )
        })?
        .to_owned();

    let context = request_context
        .with_user(authenticated_user)
        .with_actor(Actor::user(UserId::new(user_id_str.to_owned())));

    tracing::info!(
        server = %server_name,
        user_id = %user_id_str,
        "Authorized via proxy-verified identity"
    );

    Ok(Some(AuthenticatedRequestContext::new(context, token)))
}

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
