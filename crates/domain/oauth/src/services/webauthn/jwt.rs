//! WebAuthn-issued JWT validator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::TokenValidator;
use systemprompt_models::auth::{AuthError, AuthenticatedUser, JwtAudience};
use uuid::Uuid;

use crate::services::validation::jwt;

#[derive(Clone, Debug)]
pub struct JwtTokenValidator {
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtTokenValidator {
    pub const fn new(issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self { issuer, audiences }
    }

    pub fn from_config() -> Result<Self, AuthError> {
        let config =
            systemprompt_models::Config::get().map_err(|e| AuthError::AuthenticationFailed {
                message: format!("Failed to get config: {e}"),
            })?;
        Ok(Self {
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
        })
    }
}

impl TokenValidator for JwtTokenValidator {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "async signature required by the TokenValidator trait; this \
                  validator decodes the JWT synchronously"
    )]
    async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser, AuthError> {
        let claims =
            jwt::validate_jwt_token(token, &self.issuer, &self.audiences).map_err(|e| {
                AuthError::AuthenticationFailed {
                    message: format!("JWT validation failed: {e}"),
                }
            })?;

        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|e| AuthError::AuthenticationFailed {
                message: format!("Invalid user ID in token: {e}"),
            })?;

        let permissions = claims.get_permissions();
        let roles = claims.roles().to_vec();

        Ok(AuthenticatedUser::new_with_roles(
            user_id,
            claims.username.clone(),
            claims.email,
            permissions,
            roles,
        ))
    }
}
