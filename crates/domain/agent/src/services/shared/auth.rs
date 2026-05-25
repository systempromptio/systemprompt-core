//! JWT validation for agent requests: decoding bearer tokens into typed
//! session claims and extracting the authenticated [`UserId`].

use crate::services::shared::error::{AgentServiceError, Result};
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_identifiers::UserId;
pub use systemprompt_models::auth::JwtClaims;
use systemprompt_security::keys::authority;
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
        reason = "method is part of a trait surface where other impls use self"
    )]
    pub fn validate_token(&self, token: &str) -> Result<JwtClaims> {
        let header = decode_header(token)
            .map_err(|e| AgentServiceError::Authentication(format!("invalid token: {e}")))?;
        if header.alg != Algorithm::RS256 {
            return Err(AgentServiceError::Authentication(
                "JWT must be RS256-signed".to_owned(),
            ));
        }
        let kid = header.kid.as_deref().ok_or_else(|| {
            AgentServiceError::Authentication("JWT missing `kid` header".to_owned())
        })?;
        let key = authority::decoding_key_for_kid(kid)
            .map_err(|e| AgentServiceError::Authentication(format!("key lookup: {e}")))?
            .ok_or_else(|| AgentServiceError::Authentication(format!("unknown `kid` `{kid}`")))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_aud = false;
        decode::<JwtClaims>(token, key, &validation)
            .map(|data| data.claims)
            .map_err(|e| AgentServiceError::Authentication(format!("invalid token: {e}")))
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
    pub roles: Vec<String>,
}

impl AgentSessionUser {
    pub fn from_jwt_claims(claims: AgentJwtClaims) -> Self {
        Self {
            id: UserId::new(claims.subject),
            username: claims.username,
            user_type: claims.user_type,
            roles: claims.permissions,
        }
    }
}

impl From<JwtClaims> for AgentSessionUser {
    fn from(claims: JwtClaims) -> Self {
        Self {
            id: UserId::new(claims.sub.clone()),
            username: claims.username.clone(),
            user_type: claims.user_type.to_string(),
            roles: claims.get_scopes(),
        }
    }
}
