//! JWT validation and audience/scope checks for MCP RBAC.

use rmcp::ErrorData as McpError;
use systemprompt_models::auth::{JwtClaims, Permission};

use crate::services::auth::validate_jwt_token;

pub(super) fn validate_and_extract_claims(
    server_name: &str,
    token: &str,
) -> Result<JwtClaims, McpError> {
    let jwt_secret = systemprompt_config::SecretsBootstrap::jwt_secret().map_err(|e| {
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

pub(super) fn validate_audience(
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

pub(super) fn validate_scopes_for_permissions(
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
