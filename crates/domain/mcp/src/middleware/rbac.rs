//! Role-based access control for MCP server requests.
//!
//! Validates a Bearer JWT (or proxy-verified identity headers) against the
//! per-server `OAuthRequirement` declared in the registry config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use rmcp::service::RequestContext as McpContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_identifiers::{Actor, JwtToken, McpServerId, UserId};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, JwtClaims};
use systemprompt_models::execution::context::ExecutionContext;
use systemprompt_security::authz::{
    AuthzContext, AuthzDecision, AuthzRequest, EntityKind, EntityRef, SharedAuthzHook,
    member_attribute_floor,
};

use super::{extract_bearer_token, extract_request_context};

#[path = "rbac/jwt.rs"]
pub mod jwt;
#[path = "rbac/proxy.rs"]
mod proxy;

use jwt::{validate_and_extract_claims, validate_audience, validate_scopes_for_permissions};
pub use proxy::try_proxy_verified_auth;

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
) -> Result<AuthResult, McpError> {
    let header_dump = mcp_context
        .extensions
        .get::<http::request::Parts>()
        .map(diagnostic_headers);

    let services_config = ConfigLoader::load().map_err(|e| {
        tracing::error!(server = %server_id, headers = ?header_dump, error = %e, "Failed to load services config");
        McpError::internal_error("Failed to load services config", None)
    })?;

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

    let floor = member_attribute_floor(&services_config, EntityKind::McpServer, server_id.as_str());

    if let Some(proxy_auth) = try_proxy_verified_auth(
        mcp_context.extensions.get::<http::request::Parts>(),
        request_context.clone(),
        oauth_config,
        server_id,
    )? {
        let authz_request =
            proxy::build_proxy_authz_request(server_id, &proxy_auth.context, floor.as_ref());
        enforce_authz_for_server(server_id, authz_request, hook).await?;
        return Ok(AuthResult::Authenticated(proxy_auth));
    }

    let token = extract_bearer_token(mcp_context)?.ok_or_else(|| {
        tracing::error!(server = %server_id, headers = ?header_dump, "Authentication required: No Bearer token provided");
        McpError::invalid_request(
            format!(
                "Authentication required. Server '{server_id}' requires OAuth but no Bearer \
                 token provided."
            ),
            None,
        )
    })?;

    let claims = validate_and_extract_claims(server_id, &token)?;
    validate_audience(server_id, &claims, oauth_config)?;
    validate_scopes_for_permissions(server_id, &claims.get_permissions(), oauth_config)?;

    let act_chain = extract_act_chain(&claims);

    let authz_request = build_mcp_authz_request(
        server_id,
        &claims,
        act_chain.clone(),
        &request_context.execution,
        floor.as_ref(),
    );
    enforce_authz_for_server(server_id, authz_request, hook).await?;

    let authenticated_context =
        build_authenticated_context(request_context, &claims, JwtToken::new(token), act_chain)?;
    Ok(AuthResult::Authenticated(authenticated_context))
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

fn extract_act_chain(claims: &JwtClaims) -> Vec<Actor> {
    claims
        .act
        .as_ref()
        .map(systemprompt_models::auth::ActClaim::flatten_to_chain)
        .unwrap_or_default()
}

// JSON: marketplace policy floor — open attribute map passed to the authz
// context.
#[must_use]
pub fn build_mcp_authz_request(
    server_id: &McpServerId,
    claims: &JwtClaims,
    act_chain: Vec<Actor>,
    execution: &ExecutionContext,
    floor: Option<&BTreeMap<String, serde_json::Value>>,
) -> AuthzRequest {
    let context = floor.map_or_else(AuthzContext::none, |floor| {
        AuthzContext::none().with_marketplace_floor(floor)
    });
    let user_id = UserId::new(claims.sub.clone());
    AuthzRequest {
        entity: EntityRef::McpServer(server_id.clone()),
        user_id: user_id.clone(),
        actor: Some(Actor::mcp(user_id, server_id.as_str())),
        client_id: claims.client_id.clone(),
        access_scope: None,
        roles: claims.roles.clone(),
        attributes: claims.attributes.clone(),
        trace_id: execution.trace_id.clone(),
        session_id: claims.session_id.clone(),
        context,
        context_id: Some(execution.context_id.clone()),
        task_id: execution.task_id.clone(),
        act_chain,
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

fn build_authenticated_context(
    request_context: RequestContext,
    claims: &JwtClaims,
    token: JwtToken,
    act_chain: Vec<Actor>,
) -> Result<AuthenticatedRequestContext, McpError> {
    let user_id = UserId::try_new(claims.sub.clone()).map_err(|e| {
        tracing::error!(error = %e, "Invalid user ID in JWT");
        McpError::internal_error("Invalid user ID in JWT", None)
    })?;

    let authenticated_user = AuthenticatedUser::new_with_roles(
        user_id.clone(),
        claims.username.clone(),
        claims.email.clone(),
        claims.get_permissions(),
        claims.roles().to_vec(),
    );

    let context = request_context
        .with_user(authenticated_user)
        .with_actor(Actor::user(user_id))
        .with_act_chain(act_chain)
        .with_user_type(claims.user_type);

    Ok(AuthenticatedRequestContext::new(context, token))
}
