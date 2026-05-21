//! Role-based access control for MCP server requests.
//!
//! Validates a Bearer JWT (or proxy-verified identity headers) against the
//! per-server `OAuthRequirement` declared in the registry config.

use rmcp::service::RequestContext as McpContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_identifiers::{Actor, TraceId, UserId};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, JwtClaims};
use systemprompt_security::authz::{AuthzDecision, AuthzRequest, EntityKind, SharedAuthzHook};

use super::{extract_bearer_token, extract_request_context};

#[path = "rbac/jwt.rs"]
mod jwt;
#[path = "rbac/proxy.rs"]
mod proxy;

use jwt::{validate_and_extract_claims, validate_audience, validate_scopes_for_permissions};
use proxy::try_proxy_verified_auth;

#[derive(Debug, Clone)]
pub struct AuthenticatedRequestContext {
    pub context: RequestContext,
    pub auth_token: String,
}

impl AuthenticatedRequestContext {
    pub const fn new(context: RequestContext, auth_token: String) -> Self {
        Self {
            context,
            auth_token,
        }
    }

    pub fn token(&self) -> &str {
        &self.auth_token
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

    pub fn context_mut(&mut self) -> &mut RequestContext {
        match self {
            Self::Anonymous(ctx) => ctx,
            Self::Authenticated(auth_ctx) => &mut auth_ctx.context,
        }
    }

    pub fn expect_authenticated(self, msg: &str) -> Result<AuthenticatedRequestContext, McpError> {
        match self {
            Self::Authenticated(auth_ctx) => Ok(auth_ctx),
            Self::Anonymous(_) => Err(McpError::invalid_request(msg.to_string(), None)),
        }
    }
}

#[tracing::instrument(name = "mcp_rbac", skip_all)]
pub async fn enforce_rbac_from_registry(
    mcp_context: &McpContext<RoleServer>,
    server_name: &str,
    hook: &SharedAuthzHook,
) -> Result<AuthResult, McpError> {
    let header_dump = mcp_context
        .extensions
        .get::<http::request::Parts>()
        .map(|p| {
            p.headers
                .iter()
                .filter(|(k, _)| {
                    let name = k.as_str();
                    name.starts_with("x-") || name == "authorization" || name == "mcp-session-id"
                })
                .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("?")))
                .collect::<Vec<_>>()
        });

    let services_config = ConfigLoader::load().map_err(|e| {
        tracing::error!(server = %server_name, headers = ?header_dump, error = %e, "Failed to load services config");
        McpError::internal_error(format!("Failed to load services config: {e}"), None)
    })?;

    let deployment = services_config
        .mcp_servers
        .get(server_name)
        .ok_or_else(|| {
            tracing::error!(server = %server_name, headers = ?header_dump, "MCP server not found in registry");
            McpError::internal_error(
                format!("MCP server '{server_name}' not found in registry"),
                None,
            )
        })?;

    let oauth_config = &deployment.oauth;
    let request_context = extract_request_context(mcp_context)?;

    if !oauth_config.required {
        return Ok(AuthResult::Anonymous(request_context));
    }

    if let Some(auth_result) = try_proxy_verified_auth(
        mcp_context,
        request_context.clone(),
        oauth_config,
        server_name,
    )? {
        return Ok(auth_result);
    }

    let token = extract_bearer_token(mcp_context)?.ok_or_else(|| {
        tracing::error!(server = %server_name, headers = ?header_dump, "Authentication required: No Bearer token provided");
        McpError::invalid_request(
            format!(
                "Authentication required. Server '{server_name}' requires OAuth but no Bearer \
                 token provided."
            ),
            None,
        )
    })?;

    let claims = validate_and_extract_claims(server_name, &token)?;
    validate_audience(server_name, &claims, oauth_config)?;
    validate_scopes_for_permissions(server_name, &claims.get_permissions(), oauth_config)?;

    let act_chain = extract_act_chain(&claims);

    enforce_authz_for_server(server_name, &claims, act_chain.clone(), hook).await?;

    let authenticated_context =
        build_authenticated_context(request_context, &claims, token, act_chain)?;
    Ok(AuthResult::Authenticated(authenticated_context))
}

fn extract_act_chain(claims: &JwtClaims) -> Vec<Actor> {
    claims
        .act
        .as_ref()
        .map(systemprompt_models::auth::ActClaim::flatten_to_chain)
        .unwrap_or_default()
}

async fn enforce_authz_for_server(
    server_name: &str,
    claims: &JwtClaims,
    act_chain: Vec<Actor>,
    hook: &SharedAuthzHook,
) -> Result<(), McpError> {
    let user_id = UserId::new(claims.sub.clone());
    let req = AuthzRequest {
        entity_type: EntityKind::McpServer,
        entity_id: server_name.to_string(),
        user_id,
        roles: claims.roles.clone(),
        department: claims.department.clone().unwrap_or_else(String::new),
        trace_id: TraceId::generate(),
        context: serde_json::Value::Null,
        act_chain,
    };
    match hook.evaluate(req).await {
        AuthzDecision::Allow => Ok(()),
        AuthzDecision::Deny { reason, policy } => {
            tracing::warn!(server = %server_name, reason = %reason, policy = %policy, "authz hook denied MCP request");
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
    token: String,
    act_chain: Vec<Actor>,
) -> Result<AuthenticatedRequestContext, McpError> {
    let user_id = claims.sub.parse().map_err(|e| {
        tracing::error!(error = %e, "Invalid user ID in JWT");
        McpError::internal_error(format!("Invalid user ID in JWT: {e}"), None)
    })?;

    let authenticated_user = AuthenticatedUser::new_with_roles(
        user_id,
        claims.username.clone(),
        claims.email.clone(),
        claims.get_permissions(),
        claims.roles().to_vec(),
    );

    let context = request_context
        .with_user(authenticated_user)
        .with_actor(Actor::user(UserId::new(claims.sub.clone())))
        .with_act_chain(act_chain)
        .with_user_type(claims.user_type);

    Ok(AuthenticatedRequestContext::new(context, token))
}
