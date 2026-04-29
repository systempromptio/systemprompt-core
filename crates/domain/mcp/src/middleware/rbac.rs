use rmcp::service::RequestContext as McpContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_identifiers::UserId;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, JwtClaims, Permission};

use super::{extract_bearer_token, extract_request_context};
use crate::services::auth::validate_jwt_token;

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
pub fn enforce_rbac_from_registry(
    mcp_context: &McpContext<RoleServer>,
    server_name: &str,
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

    let authenticated_context = build_authenticated_context(request_context, &claims, token)?;
    Ok(AuthResult::Authenticated(authenticated_context))
}

fn try_proxy_verified_auth(
    mcp_context: &McpContext<RoleServer>,
    request_context: RequestContext,
    oauth_config: &crate::OAuthRequirement,
    server_name: &str,
) -> Result<Option<AuthResult>, McpError> {
    let parts = mcp_context
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| {
            McpError::invalid_request("No HTTP parts in MCP context".to_string(), None)
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
                "Proxy-verified request missing x-user-id header".to_string(),
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
                "Proxy-verified request missing x-user-permissions header".to_string(),
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
                "Proxy-verified request missing Authorization Bearer token".to_string(),
                None,
            )
        })?
        .to_string();

    let context = request_context
        .with_user(authenticated_user)
        .with_user_id(UserId::new(user_id_str.to_string()));

    tracing::info!(
        server = %server_name,
        user_id = %user_id_str,
        "Authorized via proxy-verified identity"
    );

    Ok(Some(AuthResult::Authenticated(
        AuthenticatedRequestContext::new(context, token),
    )))
}

fn validate_and_extract_claims(server_name: &str, token: &str) -> Result<JwtClaims, McpError> {
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret().map_err(|e| {
        tracing::error!(server = %server_name, error = %e, "Failed to get JWT secret");
        McpError::invalid_request(format!("Failed to get JWT secret: {e}"), None)
    })?;
    let config = systemprompt_models::Config::get().map_err(|e| {
        tracing::error!(server = %server_name, error = %e, "Failed to get config");
        McpError::invalid_request(format!("Failed to get config: {e}"), None)
    })?;
    validate_jwt_token(token, jwt_secret, &config.jwt_issuer, &config.jwt_audiences).map_err(|e| {
        tracing::error!(server = %server_name, error = %e, "JWT validation failed");
        McpError::invalid_request(format!("Invalid JWT token: {e}"), None)
    })
}

fn validate_audience(
    server_name: &str,
    claims: &JwtClaims,
    oauth_config: &crate::OAuthRequirement,
) -> Result<(), McpError> {
    if claims.aud.contains(&oauth_config.audience) {
        return Ok(());
    }

    tracing::error!(
        server = %server_name,
        expected = %oauth_config.audience,
        actual = ?claims.aud,
        "Invalid audience"
    );
    Err(McpError::invalid_request(
        format!(
            "Invalid audience. Expected '{}', got: {:?}",
            oauth_config.audience, claims.aud
        ),
        None,
    ))
}

fn validate_scopes_for_permissions(
    server_name: &str,
    user_permissions: &[Permission],
    oauth_config: &crate::OAuthRequirement,
) -> Result<(), McpError> {
    let required_scopes = &oauth_config.scopes;

    let has_required_scope = required_scopes.iter().any(|required| {
        user_permissions
            .iter()
            .any(|user_perm| user_perm.implies(required))
    });

    if has_required_scope {
        return Ok(());
    }

    tracing::error!(
        server = %server_name,
        required = ?required_scopes,
        user_permissions = ?user_permissions,
        "Insufficient permissions"
    );
    Err(McpError::invalid_request(
        format!(
            "Insufficient permissions. User must have one of: {required_scopes:?}, but has: \
             {user_permissions:?}"
        ),
        None,
    ))
}

fn build_authenticated_context(
    request_context: RequestContext,
    claims: &JwtClaims,
    token: String,
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
        .with_user_id(UserId::new(claims.sub.clone()))
        .with_user_type(claims.user_type);

    Ok(AuthenticatedRequestContext::new(context, token))
}
