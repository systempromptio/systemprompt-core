//! Token extraction and validation for A2A requests.
//!
//! [`extract_bearer_token`] pulls the bearer credential from request headers;
//! [`validate_oauth_for_request`] verifies the token through the kernel's
//! request authenticator (the `a2a` audience, the user row, the session row
//! and revocation) and enforces the required permission scopes: the token's
//! scopes, with `admin` (and the admin bypass) honoured only while the user
//! row still holds the admin role.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::{HeaderMap, StatusCode};
use serde::Serialize;
use std::str::FromStr;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_models::auth::{
    DynRequestAuthenticator, JwtAudience, Permission, UserRole, UserType,
};
use systemprompt_models::execution::context::ContextExtractionError;

use crate::services::a2a_server::errors::{
    JsonRpcErrorResponse, forbidden_response, unauthorized_response,
};

/// Identity of an authenticated A2A caller: the user row, and the token's
/// scopes as the user row still admits them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthenticatedCaller {
    pub sub: UserId,
    pub username: String,
    pub user_type: String,
    pub is_admin: bool,
    pub permissions: Vec<String>,
    pub audiences: Vec<String>,
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
    authenticator: Option<&DynRequestAuthenticator>,
) -> Result<Option<AuthenticatedCaller>, Box<(StatusCode, JsonRpcErrorResponse)>> {
    let token = match extract_bearer_token(headers) {
        Some(t) if !t.is_empty() => JwtToken::new(t),
        _ => {
            return Err(Box::new(unauthorized_response(
                "Bearer token required. Include 'Authorization: Bearer <token>' header.",
                request_id,
            )));
        },
    };

    let Some(authenticator) = authenticator else {
        return Err(Box::new(unauthorized_response(
            "Request authenticator not configured",
            request_id,
        )));
    };

    let caller = match authenticate_caller(authenticator, &token).await {
        Ok(caller) => caller,
        Err(e) => {
            return Err(Box::new(unauthorized_response(
                format!("Invalid or expired token: {e}"),
                request_id,
            )));
        },
    };

    tracing::info!(
        username = %caller.username,
        user_type = %caller.user_type,
        "Authenticated"
    );

    if caller.is_admin {
        tracing::info!(
            username = %caller.username,
            "Admin user has access to all agents"
        );
        return Ok(Some(caller));
    }

    if let Some(reason) = missing_scopes_reason(&caller, required_scopes) {
        return Err(Box::new(forbidden_response(reason, request_id)));
    }

    Ok(Some(caller))
}

async fn authenticate_caller(
    authenticator: &DynRequestAuthenticator,
    token: &JwtToken,
) -> Result<AuthenticatedCaller, ContextExtractionError> {
    let (claims, user) = authenticator
        .decode_for_audiences(token, &[JwtAudience::A2a])
        .await?;
    let user_is_admin = user
        .roles
        .iter()
        .any(|role| role.as_str() == UserRole::Admin.as_str());
    let user_type = claims.user_type.reconcile_with(user_is_admin);
    Ok(AuthenticatedCaller {
        sub: user.id,
        username: user.name,
        user_type: user_type.to_string(),
        is_admin: user_type == UserType::Admin,
        permissions: claims
            .scope
            .iter()
            .filter(|scope| user_is_admin || **scope != Permission::Admin)
            .map(ToString::to_string)
            .collect(),
        audiences: vec![JwtAudience::A2a.as_str().to_owned()],
    })
}

fn missing_scopes_reason(
    claims: &AuthenticatedCaller,
    required_scopes: &[Permission],
) -> Option<String> {
    if required_scopes.is_empty() {
        tracing::warn!(
            username = %claims.username,
            "Access denied: agent requires OAuth but declares no scopes"
        );
        return Some("Agent requires OAuth but declares no scopes".to_owned());
    }

    let has_required_scope = required_scopes.iter().any(|required_scope| {
        claims.permissions.iter().any(|user_perm| {
            Permission::from_str(user_perm).is_ok_and(|p| p.grants(required_scope))
        })
    });

    if has_required_scope {
        return None;
    }

    let required_scopes_str: Vec<String> =
        required_scopes.iter().map(ToString::to_string).collect();

    tracing::warn!(
        username = %claims.username,
        required = %required_scopes_str.join(", "),
        has = %claims.permissions.join(", "),
        "Access denied: User lacks required scopes"
    );

    Some(format!(
        "User {} lacks required permissions. Required: [{}], User has: [{}]",
        claims.username,
        required_scopes_str.join(", "),
        claims.permissions.join(", ")
    ))
}
