//! Token extraction and validation for A2A requests.
//!
//! [`extract_bearer_token`] pulls the bearer credential from request headers;
//! [`validate_agent_token`] and [`validate_oauth_for_request`] verify the JWT,
//! confirm the `a2a` audience, and enforce the required permission scopes.

use crate::services::shared::{AgentServiceError, Result};
use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;
use systemprompt_models::auth::Permission;
use systemprompt_traits::AgentJwtClaims;

use super::types::AgentOAuthState;
use crate::services::a2a_server::errors::{forbidden_response, unauthorized_response};
use crate::services::shared::AgentSessionUser;

pub async fn validate_agent_token(
    token: &str,
    state: &AgentOAuthState,
) -> Result<AgentSessionUser> {
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

    Ok(AgentSessionUser::from_jwt_claims(claims))
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

    let claims = match provider.validate_token(&token) {
        Ok(claims) => claims,
        Err(e) => {
            return Err(unauthorized_response(
                format!("Invalid or expired token: {e}"),
                request_id,
            ));
        },
    };

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
        return Ok(Some(claims_payload(&claims)));
    }

    ensure_required_scopes(&claims, required_scopes, request_id)?;

    Ok(Some(claims_payload(&claims)))
}

fn ensure_required_scopes(
    claims: &AgentJwtClaims,
    required_scopes: &[Permission],
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
) -> std::result::Result<(), (StatusCode, serde_json::Value)> {
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

    Ok(())
}

fn claims_payload(claims: &AgentJwtClaims) -> serde_json::Value {
    serde_json::json!({
        "sub": claims.subject,
        "username": claims.username,
        "user_type": claims.user_type,
        "is_admin": claims.is_admin,
        "permissions": claims.permissions,
        "audiences": claims.audiences
    })
}
