use crate::services::shared::{AgentServiceError, Result};
use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::Permission;
use systemprompt_traits::{AgentJwtClaims, AuthUser};

use super::types::{AgentAuthenticatedUser, AgentOAuthState};
use crate::services::a2a_server::errors::{forbidden_response, unauthorized_response};

pub async fn validate_agent_token(
    token: &str,
    state: &AgentOAuthState,
) -> Result<AgentAuthenticatedUser> {
    let jwt_provider = state
        .jwt_provider
        .as_ref()
        .ok_or_else(|| AgentServiceError::Internal("JWT provider not configured".to_owned()))?;

    let claims = jwt_provider
        .validate_token(token)
        .map_err(|e| AgentServiceError::Internal(format!("Invalid or expired JWT token: {e}")))?;

    if !claims.has_audience("a2a") {
        return Err(AgentServiceError::Internal(
            "Token does not support A2A protocol".to_owned(),
        ));
    }

    if let Some(ref user_provider) = state.user_provider {
        let user = verify_user_exists_and_active(&claims, user_provider.as_ref()).await?;
        verify_a2a_permissions(&claims, &user)?;

        tracing::debug!(
            username = %claims.username,
            user_type = %claims.user_type,
            is_active = user.is_active,
            "Authenticated A2A user"
        );
    }

    Ok(AgentAuthenticatedUser::from_jwt_claims(claims))
}

async fn verify_user_exists_and_active(
    claims: &AgentJwtClaims,
    user_provider: &dyn systemprompt_traits::UserProvider,
) -> Result<AuthUser> {
    let subject_id = UserId::new(&claims.subject);
    let user = user_provider.find_by_id(&subject_id).await.map_err(|e| {
        AgentServiceError::Internal(format!("Failed to lookup user in database: {e}"))
    })?;

    let Some(user) = user else {
        tracing::warn!(
            user_id = %claims.subject,
            "User ID from token not found in database"
        );
        return Err(AgentServiceError::Internal("User not found".to_owned()));
    };

    if !user.is_active {
        tracing::warn!(
            username = %claims.username,
            is_active = user.is_active,
            "User has non-active status"
        );
        return Err(AgentServiceError::Internal(
            "User account is not active".to_owned(),
        ));
    }

    Ok(user)
}

fn verify_a2a_permissions(claims: &AgentJwtClaims, user: &AuthUser) -> Result<()> {
    let token_has_admin_permission = claims.is_admin || claims.has_permission("admin");

    let db_permissions: Vec<Permission> = user
        .roles
        .iter()
        .filter_map(|role| {
            Permission::from_str(role)
                .map_err(|e| {
                    tracing::debug!(role = %role, error = %e, "Unknown permission role, skipping");
                    e
                })
                .ok()
        })
        .collect();

    if db_permissions.is_empty() {
        return Err(AgentServiceError::Internal(format!(
            "User {} has no valid permissions",
            user.id
        )));
    }

    let db_has_admin_permission = db_permissions.contains(&Permission::Admin);

    if !token_has_admin_permission && !db_has_admin_permission {
        return Err(AgentServiceError::Internal(
            "User lacks required A2A permissions".to_owned(),
        ));
    }

    Ok(())
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|value| {
            value
                .to_str()
                .map_err(|e| {
                    tracing::debug!(error = %e, "Authorization header contains non-ASCII characters");
                    e
                })
                .ok()
        })
        .and_then(|auth_header| {
            auth_header
                .strip_prefix("Bearer ")
                .map(str::to_owned)
        })
}

pub async fn validate_oauth_for_request(
    headers: &HeaderMap,
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
    required_scopes: &[Permission],
    jwt_provider: Option<&std::sync::Arc<dyn systemprompt_traits::JwtValidationProvider>>,
) -> std::result::Result<Option<serde_json::Value>, (StatusCode, serde_json::Value)> {
    let token = match extract_bearer_token(headers) {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Err(unauthorized_response(
                "Bearer token required. Include 'Authorization: Bearer <token>' header.",
                request_id,
            ));
        },
    };

    let Some(provider) = jwt_provider else {
        return Err(unauthorized_response(
            "JWT provider not configured",
            request_id,
        ));
    };

    match provider.validate_token(&token) {
        Ok(claims) => {
            tracing::info!(
                username = %claims.username,
                user_type = %claims.user_type,
                "Authenticated"
            );

            if !claims.has_audience("a2a") {
                return Err(forbidden_response(
                    format!(
                        "Token does not support A2A protocol. Audience: {:?}",
                        claims.audiences
                    ),
                    request_id,
                ));
            }

            if claims.is_admin {
                tracing::info!(
                    username = %claims.username,
                    "Admin user has access to all agents"
                );
                return Ok(Some(serde_json::json!({
                    "sub": claims.subject,
                    "username": claims.username,
                    "user_type": claims.user_type,
                    "is_admin": claims.is_admin,
                    "permissions": claims.permissions,
                    "audiences": claims.audiences
                })));
            }

            let has_required_scope = required_scopes.iter().any(|required_scope| {
                claims.permissions.iter().any(|user_perm| {
                    Permission::from_str(user_perm).is_ok_and(|p| p.implies(required_scope))
                })
            });

            if !has_required_scope {
                let required_scopes_str: Vec<String> =
                    required_scopes.iter().map(ToString::to_string).collect();

                tracing::warn!(
                    username = %claims.username,
                    required = %required_scopes_str.join(", "),
                    has = %claims.permissions.join(", "),
                    "Access denied: User lacks required scopes"
                );

                return Err(forbidden_response(
                    format!(
                        "User {} lacks required permissions. Required: [{}], User has: [{}]",
                        claims.username,
                        required_scopes_str.join(", "),
                        claims.permissions.join(", ")
                    ),
                    request_id,
                ));
            }

            Ok(Some(serde_json::json!({
                "sub": claims.subject,
                "username": claims.username,
                "user_type": claims.user_type,
                "is_admin": claims.is_admin,
                "permissions": claims.permissions,
                "audiences": claims.audiences
            })))
        },
        Err(e) => Err(unauthorized_response(
            format!("Invalid or expired token: {e}"),
            request_id,
        )),
    }
}
