//! Scope checks for MCP RBAC.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::auth::Permission;

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
            .any(|user_perm| user_perm.grants(required))
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
