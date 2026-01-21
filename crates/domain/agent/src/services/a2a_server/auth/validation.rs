use anyhow::{Context, Result};
use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;
use systemprompt_identifiers::SessionId;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_traits::{AgentJwtClaims, AuthUser, GenerateTokenParams};

use super::types::{AgentAuthenticatedUser, AgentOAuthState};
use crate::services::a2a_server::errors::{forbidden_response, unauthorized_response};

pub async fn validate_agent_token(
    token: &str,
    state: &AgentOAuthState,
) -> Result<AgentAuthenticatedUser> {
    let jwt_provider = state
        .jwt_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("JWT provider not configured"))?;

    let claims = jwt_provider
        .validate_token(token)
        .map_err(|e| anyhow::anyhow!("Invalid or expired JWT token: {}", e))?;

    if !claims.has_audience("a2a") {
        return Err(anyhow::anyhow!("Token does not support A2A protocol"));
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

pub async fn generate_agent_token(
    user_id: &str,
    username: &str,
    state: &AgentOAuthState,
) -> Result<String> {
    let jwt_provider = state
        .jwt_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("JWT provider not configured"))?;

    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));

    let params = GenerateTokenParams::new(user_id, username, session_id)
        .with_permissions(vec!["a2a".to_string()])
        .with_audiences(vec!["a2a".to_string()])
        .with_expires_in_hours(1);

    jwt_provider
        .generate_token(params)
        .map_err(|e| anyhow::anyhow!("Failed to generate A2A JWT token: {}", e))
}

pub async fn generate_cross_protocol_token(
    user_id: &str,
    username: &str,
    state: &AgentOAuthState,
) -> Result<String> {
    let jwt_provider = state
        .jwt_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("JWT provider not configured"))?;

    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));

    let params = GenerateTokenParams::new(user_id, username, session_id)
        .with_permissions(vec!["mcp".to_string(), "a2a".to_string()])
        .with_audiences(vec!["mcp".to_string(), "a2a".to_string()])
        .with_expires_in_hours(1);

    jwt_provider
        .generate_token(params)
        .map_err(|e| anyhow::anyhow!("Failed to generate cross-protocol JWT token: {}", e))
}

async fn verify_user_exists_and_active(
    claims: &AgentJwtClaims,
    user_provider: &dyn systemprompt_traits::UserProvider,
) -> Result<AuthUser> {
    let user = user_provider
        .find_by_id(&claims.subject)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to lookup user in database: {}", e))?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!(
                user_id = %claims.subject,
                "User ID from token not found in database"
            );
            return Err(anyhow::anyhow!("User not found"));
        },
    };

    if !user.is_active {
        tracing::warn!(
            username = %claims.username,
            is_active = user.is_active,
            "User has non-active status"
        );
        return Err(anyhow::anyhow!("User account is not active"));
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
        return Err(anyhow::anyhow!("User {} has no valid permissions", user.id));
    }

    let db_has_admin_permission = db_permissions.contains(&Permission::Admin);

    if !token_has_admin_permission && !db_has_admin_permission {
        return Err(anyhow::anyhow!("User lacks required A2A permissions"));
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
            if auth_header.starts_with("Bearer ") {
                Some(auth_header[7..].to_string())
            } else {
                None
            }
        })
}

pub async fn validate_oauth_for_request(
    headers: &HeaderMap,
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
    required_scopes: &[Permission],
    jwt_provider: Option<&std::sync::Arc<dyn systemprompt_traits::JwtValidationProvider>>,
) -> Result<Option<serde_json::Value>, (StatusCode, serde_json::Value)> {
    let token = match extract_bearer_token(headers) {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Err(unauthorized_response(
                "Bearer token required. Include 'Authorization: Bearer <token>' header.",
                request_id,
            )
            .await);
        },
    };

    let provider = match jwt_provider {
        Some(p) => p,
        None => {
            return Err(
                unauthorized_response("JWT provider not configured", request_id).await
            );
        },
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
                )
                .await);
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
                claims
                    .permissions
                    .iter()
                    .any(|user_perm| {
                        Permission::from_str(user_perm)
                            .map(|p| p.implies(required_scope))
                            .unwrap_or(false)
                    })
            });

            if !has_required_scope {
                let required_scopes_str: Vec<String> =
                    required_scopes.iter().map(|s| s.to_string()).collect();

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
                )
                .await);
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
        Err(e) => {
            Err(unauthorized_response(format!("Invalid or expired token: {e}"), request_id).await)
        },
    }
}
