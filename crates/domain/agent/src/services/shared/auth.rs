//! JWT validation for agent requests: decoding bearer tokens into typed
//! session claims and extracting the authenticated [`UserId`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::error::{AgentServiceError, Result};
use systemprompt_identifiers::UserId;
pub use systemprompt_models::auth::JwtClaims;
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};
use systemprompt_traits::AgentJwtClaims;

#[derive(Debug, Default, Clone, Copy)]
pub struct JwtValidator;

impl JwtValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[expect(
        clippy::unused_self,
        reason = "trait-shaped method kept on impl for symmetry"
    )]
    pub fn validate_token(&self, token: &str) -> Result<JwtClaims> {
        decode_rs256_claims(token, &ValidationPolicy::session_context())
            .map_err(|e| AgentServiceError::Authentication(e.to_string()))
    }
}

pub fn extract_bearer_token(authorization_header: &str) -> Result<&str> {
    authorization_header.strip_prefix("Bearer ").ok_or_else(|| {
        AgentServiceError::Authentication("invalid authorization header format".to_owned())
    })
}

#[derive(Debug, Clone)]
pub struct AgentSessionUser {
    pub id: UserId,
    pub username: String,
    pub user_type: String,
    pub permissions: Vec<String>,
}

impl AgentSessionUser {
    pub fn from_jwt_claims(claims: AgentJwtClaims) -> Self {
        Self {
            id: UserId::new(claims.subject),
            username: claims.username,
            user_type: claims.user_type,
            permissions: claims.permissions,
        }
    }
}
