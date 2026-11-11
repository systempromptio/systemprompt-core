//! Role-based access control for MCP server requests.
//!
//! Authorises a request against the per-server `OAuthRequirement` declared in
//! the registry config. The caller's identity is the one the hop layer
//! verified from the proxy's signed hop token ([`VerifiedHop`]); a server that
//! requires OAuth refuses any request without one, and no header can stand in
//! for it. The caller's own bearer travels alongside only as the credential a
//! tool may relay onward. The caller
//! passes the services config it already holds (the server's services watch),
//! so a request never re-reads the services tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use rmcp::service::RequestContext as McpContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_governance::authz::{
    AuthzContext, AuthzDecision, AuthzRequest, EntityKind, EntityRef, SharedAuthzHook,
    member_attribute_floor,
};
use systemprompt_identifiers::{Actor, JwtToken, McpServerId};
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::RequestContext;

use super::extract_request_context;

#[path = "rbac/scopes.rs"]
pub mod scopes;

use super::hop::VerifiedHop;
use scopes::validate_scopes_for_permissions;

#[derive(Debug, Clone)]
pub struct AuthenticatedRequestContext {
    pub context: RequestContext,
}

impl AuthenticatedRequestContext {
    pub fn new(context: RequestContext, auth_token: JwtToken) -> Self {
        Self {
            context: context.with_auth_token(auth_token),
        }
    }
}

impl std::ops::Deref for AuthenticatedRequestContext {
    type Target = RequestContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

#[derive(Debug)]
pub enum AuthResult {
    Anonymous(RequestContext),
    Authenticated(AuthenticatedRequestContext),
}

impl AuthResult {
    pub const fn context(&self) -> &RequestContext {
        match self {
            Self::Anonymous(ctx) => ctx,
            Self::Authenticated(auth_ctx) => &auth_ctx.context,
        }
    }

    pub const fn context_mut(&mut self) -> &mut RequestContext {
        match self {
            Self::Anonymous(ctx) => ctx,
            Self::Authenticated(auth_ctx) => &mut auth_ctx.context,
        }
    }

    pub fn expect_authenticated(self, msg: &str) -> Result<AuthenticatedRequestContext, McpError> {
        match self {
            Self::Authenticated(auth_ctx) => Ok(auth_ctx),
            Self::Anonymous(_) => Err(McpError::invalid_request(msg.to_owned(), None)),
        }
    }
}

#[tracing::instrument(name = "mcp_rbac", skip_all)]
pub async fn enforce_rbac_from_registry(
    mcp_context: &McpContext<RoleServer>,
    server_id: &McpServerId,
    hook: &SharedAuthzHook,
    services_config: &ServicesConfig,
) -> Result<AuthResult, McpError> {
    let parts = mcp_context.extensions.get::<http::request::Parts>();
    let header_dump = parts.map(diagnostic_headers);

    let deployment = services_config
        .mcp_servers
        .get(server_id.as_str())
        .ok_or_else(|| {
            tracing::error!(server = %server_id, headers = ?header_dump, "MCP server not found in registry");
            McpError::internal_error(
                format!("MCP server '{server_id}' not found in registry"),
                None,
            )
        })?;

    let oauth_config = &deployment.oauth;
    let request_context = extract_request_context(mcp_context)?;

    if !oauth_config.required {
        return Ok(AuthResult::Anonymous(request_context));
    }

    let hop = parts
        .and_then(|parts| parts.extensions.get::<VerifiedHop>())
        .ok_or_else(|| {
            tracing::warn!(server = %server_id, headers = ?header_dump, "Authentication required: no verified hop token");
            McpError::invalid_request(
                format!(
                    "Authentication required. Server '{server_id}' accepts requests only through \
                     the platform proxy."
                ),
                None,
            )
        })?;
    let user = hop.claims().identity.user.as_ref().ok_or_else(|| {
        McpError::invalid_request(
            format!(
                "Authentication required. Server '{server_id}' requires an authenticated user."
            ),
            None,
        )
    })?;
    validate_scopes_for_permissions(server_id, &user.permissions, oauth_config)?;

    let floor = member_attribute_floor(services_config, EntityKind::McpServer, server_id.as_str());
    let authz_request = build_hop_authz_request(server_id, &request_context, floor.as_ref());
    enforce_authz_for_server(server_id, authz_request, hook).await?;

    let relay = request_context.auth_token().cloned().ok_or_else(|| {
        McpError::invalid_request(
            format!("Server '{server_id}' received no caller credential to relay."),
            None,
        )
    })?;
    tracing::debug!(server = %server_id, user_id = %user.id, "Authorized via the proxy hop token");
    Ok(AuthResult::Authenticated(AuthenticatedRequestContext::new(
        request_context,
        relay,
    )))
}

// Why: the dump exists to diagnose failed auth; the bearer credential must
// never reach the log, so only its presence is recorded.
fn diagnostic_headers(parts: &http::request::Parts) -> Vec<String> {
    parts
        .headers
        .iter()
        .filter(|(k, _)| {
            let name = k.as_str();
            name.starts_with("x-") || name == "authorization" || name == "mcp-session-id"
        })
        .map(|(k, v)| {
            if k == http::header::AUTHORIZATION {
                format!("{k}: <redacted>")
            } else {
                format!("{k}: {}", v.to_str().unwrap_or("?"))
            }
        })
        .collect()
}

// JSON: marketplace policy floor — open attribute map passed to the authz
// context.
#[must_use]
pub fn build_hop_authz_request(
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

async fn enforce_authz_for_server(
    server_id: &McpServerId,
    req: AuthzRequest,
    hook: &SharedAuthzHook,
) -> Result<(), McpError> {
    match hook.evaluate(req).await {
        AuthzDecision::Allow => Ok(()),
        AuthzDecision::Deny { reason, policy } => {
            tracing::warn!(server = %server_id, reason = %reason, policy = %policy, "authz hook denied MCP request");
            Err(McpError::invalid_request(
                format!("authz denied [{policy}]: {reason}"),
                None,
            ))
        },
    }
}
