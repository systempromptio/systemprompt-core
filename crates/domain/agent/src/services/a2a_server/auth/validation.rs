use anyhow::{Context, Result};
use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;
use systemprompt_oauth::services::{
    generate_jwt, generate_secure_token, JwtConfig, JwtSigningParams,
};
use systemprompt_oauth::validate_jwt_token;
use systemprompt_users::UserService;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};
use uuid;

use super::types::{AgentAuthenticatedUser, AgentOAuthState};
use crate::services::a2a_server::errors::{forbidden_response, unauthorized_response};

pub async fn validate_agent_token(
    token: &str,
    state: &AgentOAuthState,
) -> Result<AgentAuthenticatedUser> {
    let jwt_secret =
        systemprompt_models::SecretsBootstrap::jwt_secret().context("Failed to get JWT secret")?;
    let config = systemprompt_models::Config::get().context("Failed to get config")?;
    let claims = validate_jwt_token(token, jwt_secret, &config.jwt_issuer, &config.jwt_audiences)
        .context("Invalid or expired JWT token")?;

    if !claims.has_audience(JwtAudience::A2a) {
        return Err(anyhow::anyhow!("Token does not support A2A protocol"));
    }

    let user_service = UserService::new(&state.db)?;
    let user = verify_user_exists_and_active(&claims, &user_service).await?;

    verify_a2a_permissions(&claims, &user)?;

    let db_roles = user.roles.clone();
    let status = user.status.as_deref().unwrap_or("unknown");

    tracing::debug!(
        username = %claims.username,
        user_type = %claims.user_type,
        status = %status,
        db_roles = ?db_roles,
        "Authenticated A2A user"
    );

    Ok(AgentAuthenticatedUser::from(claims))
}

pub async fn generate_agent_token(
    user_context: &AuthenticatedUser,
    _state: &AgentOAuthState,
) -> Result<String> {
    let jti = generate_secure_token("a2a");

    let config = JwtConfig {
        permissions: vec![Permission::A2a],
        audience: vec![JwtAudience::A2a],
        expires_in_hours: Some(1),
    };
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let jwt_secret =
        systemprompt_models::SecretsBootstrap::jwt_secret().context("Failed to get JWT secret")?;
    let global_config = systemprompt_models::Config::get().context("Failed to get config")?;
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    generate_jwt(user_context, config, jti, &session_id, &signing)
        .context("Failed to generate A2A JWT token")
}

pub async fn generate_cross_protocol_token(
    user_context: &AuthenticatedUser,
    _state: &AgentOAuthState,
) -> Result<String> {
    let jti = generate_secure_token("cross");

    let config = JwtConfig {
        permissions: vec![Permission::Mcp, Permission::A2a],
        audience: vec![JwtAudience::Mcp, JwtAudience::A2a],
        expires_in_hours: Some(1),
    };
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let jwt_secret =
        systemprompt_models::SecretsBootstrap::jwt_secret().context("Failed to get JWT secret")?;
    let global_config = systemprompt_models::Config::get().context("Failed to get config")?;
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    generate_jwt(user_context, config, jti, &session_id, &signing)
        .context("Failed to generate cross-protocol JWT token")
}

async fn verify_user_exists_and_active(
    claims: &systemprompt_oauth::JwtClaims,
    user_service: &UserService,
) -> Result<systemprompt_users::models::User> {
    let user_id = UserId::new(claims.sub.clone());
    let user = user_service
        .find_by_id(&user_id)
        .await
        .context("Failed to lookup user in database")?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!(
                user_id = %claims.sub,
                "User ID from token not found in database"
            );
            return Err(anyhow::anyhow!("User not found"));
        },
    };

    let status = user.status.as_deref().unwrap_or("unknown");
    if status != "active" {
        tracing::warn!(
            username = %claims.username,
            status = %status,
            "User has non-active status"
        );
        return Err(anyhow::anyhow!("User account is not active"));
    }

    Ok(user)
}

fn verify_a2a_permissions(
    claims: &systemprompt_oauth::JwtClaims,
    user: &systemprompt_users::models::User,
) -> Result<()> {
    let token_has_a2a_permission = claims.has_permission(Permission::Admin);

    let db_permissions: Vec<Permission> = user
        .roles
        .iter()
        .filter_map(|role| Permission::from_str(role).ok())
        .collect();

    if db_permissions.is_empty() {
        return Err(anyhow::anyhow!("User {} has no valid permissions", user.id));
    }

    let db_has_a2a_permission = db_permissions.contains(&Permission::Admin);

    if !token_has_a2a_permission && !db_has_a2a_permission {
        return Err(anyhow::anyhow!("User lacks required A2A permissions"));
    }

    Ok(())
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
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

    let jwt_secret = match systemprompt_models::SecretsBootstrap::jwt_secret() {
        Ok(s) => s,
        Err(e) => {
            return Err(unauthorized_response(
                format!("Failed to get JWT secret: {}", e),
                request_id,
            )
            .await);
        },
    };
    let config = match systemprompt_models::Config::get() {
        Ok(c) => c,
        Err(e) => {
            return Err(
                unauthorized_response(format!("Failed to get config: {}", e), request_id).await,
            );
        },
    };
    match validate_jwt_token(
        &token,
        jwt_secret,
        &config.jwt_issuer,
        &config.jwt_audiences,
    ) {
        Ok(claims) => {
            tracing::info!(
                username = %claims.username,
                user_type = %claims.user_type,
                "Authenticated"
            );

            if !claims.has_audience(JwtAudience::A2a) {
                return Err(forbidden_response(
                    format!(
                        "Token does not support A2A protocol. Audience: {:?}",
                        claims.aud
                    ),
                    request_id,
                )
                .await);
            }

            if claims.is_admin() {
                tracing::info!(
                    username = %claims.username,
                    "Admin user has access to all agents"
                );
                return Ok(Some(serde_json::json!(claims)));
            }

            let user_permissions = claims.permissions();
            let has_required_scope = required_scopes.iter().any(|required_scope| {
                user_permissions
                    .iter()
                    .any(|user_perm| user_perm.implies(required_scope))
            });

            if !has_required_scope {
                let required_scopes_str: Vec<String> =
                    required_scopes.iter().map(|s| s.to_string()).collect();
                let user_scopes_str: Vec<String> =
                    user_permissions.iter().map(|s| s.to_string()).collect();

                tracing::warn!(
                    username = %claims.username,
                    required = %required_scopes_str.join(", "),
                    has = %user_scopes_str.join(", "),
                    "Access denied: User lacks required scopes"
                );

                return Err(forbidden_response(
                    format!(
                        "User {} lacks required permissions. Required: [{}], User has: [{}]",
                        claims.username,
                        required_scopes_str.join(", "),
                        user_scopes_str.join(", ")
                    ),
                    request_id,
                )
                .await);
            }

            Ok(Some(serde_json::json!(claims)))
        },
        Err(e) => {
            Err(unauthorized_response(format!("Invalid or expired token: {e}"), request_id).await)
        },
    }
}
