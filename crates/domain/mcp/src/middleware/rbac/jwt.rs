//! JWT validation and audience/scope checks for MCP RBAC.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::auth::{JwtClaims, Permission};

use crate::services::auth::validate_jwt_token;

pub fn validate_and_extract_claims(
    server_id: &McpServerId,
    token: &str,
) -> Result<JwtClaims, McpError> {
    let config = systemprompt_manifest::Config::get().map_err(|e| {
        tracing::error!(server = %server_id, error = %e, "Failed to get config");
        McpError::internal_error("Failed to get config", None)
    })?;
    validate_jwt_token(token, &config.jwt_issuer, &config.jwt_audiences).map_err(|e| {
        tracing::error!(server = %server_id, error = %e, "JWT validation failed");
        McpError::invalid_request(format!("Invalid JWT token: {e}"), None)
    })
}

pub fn validate_audience(
    server_id: &McpServerId,
    claims: &JwtClaims,
    oauth_config: &crate::OAuthRequirement,
) -> Result<(), McpError> {
    if claims.aud.contains(&oauth_config.audience) {
        return Ok(());
    }

    tracing::error!(
        server = %server_id,
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

pub fn validate_scopes_for_permissions(
    server_id: &McpServerId,
    user_permissions: &[Permission],
    oauth_config: &crate::OAuthRequirement,
) -> Result<(), McpError> {
    let required_scopes = &oauth_config.scopes;

    if required_scopes.is_empty() {
        tracing::error!(server = %server_id, "OAuth required but no scopes are declared");
        return Err(McpError::invalid_request(
            format!("MCP server {server_id} requires OAuth but declares no scopes"),
            None,
        ));
    }

    let has_required_scope = required_scopes.iter().any(|required| {
        user_permissions
            .iter()
            .any(|user_perm| user_perm.implies(required))
    });

    if has_required_scope {
        return Ok(());
    }

    tracing::error!(
        server = %server_id,
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
