//! Token extraction and validation for A2A requests.
//!
//! [`extract_bearer_token`] pulls the bearer credential from request headers;
//! [`validate_oauth_for_request`] verifies the JWT, confirms the `a2a`
//! audience, and enforces the required permission scopes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;
use systemprompt_models::auth::Permission;
use systemprompt_traits::AgentJwtClaims;

use crate::services::a2a_server::errors::{forbidden_response, unauthorized_response};

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
) -> Result<Option<serde_json::Value>, (StatusCode, serde_json::Value)> {
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
) -> Result<(), (StatusCode, serde_json::Value)> {
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
